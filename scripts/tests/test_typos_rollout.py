"""Tests for the repository spelling-policy scripts."""

from __future__ import annotations

import ast
import json
import os
import tomllib
import typing as typ
import urllib.error
import urllib.request
from pathlib import Path

import pytest

if typ.TYPE_CHECKING:
    import types

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]


def test_rollout_integration_contract() -> None:
    """Rollout scripts parse and the spelling gate requires indexed config."""
    for script in SCRIPT_DIRECTORY.glob("*.py"):
        ast.parse(
            script.read_text(encoding="utf-8"),
            filename=str(script),
            feature_version=(3, 13),
        )
    makefile = SCRIPT_DIRECTORY.parent.joinpath("Makefile").read_text(encoding="utf-8")
    assert "git ls-files --error-unmatch typos.toml >/dev/null" in makefile, (
        "the spelling gate must require generated config to be tracked"
    )


def _dictionary_text(stem: str = "organ") -> str:
    """Return a minimal valid shared-dictionary document."""
    return (
        'schema = 1\n\n[oxford]\nstems = ["'
        + stem
        + '"]\n\n[words]\naccepted = []\n\n[words.corrections]\n\n'
        + "[patterns]\nignore = []\n\n[files]\nexclude = []\n"
    )


def test_rollout_generates_oxford_corrections(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
) -> None:
    """The shared renderer accepts Oxford forms and corrects plain-British ones."""
    _, rollout, _ = rollout_modules

    mappings = rollout.generate_word_mappings(rollout.Dictionary(stems=("organ",)))

    assert mappings["organize"] == "organize", "Oxford spelling must be accepted"
    assert mappings["organise"] == "organize", "plain British spelling must correct"
    assert mappings["organisably"] == "organizably", "-isably must correct to -izably"


def test_local_refresh_keeps_a_newer_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """An older local authority cannot replace a newer untracked cache."""
    _, rollout, _ = rollout_modules
    source = tmp_path / "shared.toml"
    cache = tmp_path / ".typos-base.toml"
    metadata = tmp_path / ".typos-base.json"
    source.write_text(_dictionary_text(), encoding="utf-8")
    source.touch()
    rollout.refresh_base(source, cache, metadata=metadata)
    cache.write_text(_dictionary_text("newer"), encoding="utf-8")
    cache.touch()
    source_mtime = source.stat().st_mtime_ns
    cache_mtime = max(cache.stat().st_mtime_ns, source_mtime + 1)
    os.utime(cache, ns=(cache_mtime, cache_mtime))

    result = rollout.refresh_base(source, cache, metadata=metadata)

    assert result.status == "current", "older authority must preserve the cache"
    assert rollout.load_dictionary(cache).stems == ("newer",), (
        "newer cached policy must remain installed"
    )


def test_https_failure_reuses_valid_tracked_config(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """A clean network-restricted checkout retains its reviewed policy."""
    _, rollout, generator = rollout_modules
    tracked_config = tmp_path / "typos.toml"
    tracked_config.write_text('[default]\nlocale = "en-gb"\n', encoding="utf-8")

    def unavailable(*_args: object, **_kwargs: object) -> None:
        """Model an unavailable HTTPS authority."""
        raise rollout.NetworkUnavailableError("offline")

    monkeypatch.setattr(rollout, "refresh_base", unavailable)

    result = generator.main(repository=tmp_path, source="https://example.invalid/base")

    assert result.status == "tracked-config", "connectivity failure permits fallback"
    assert result.cache == tracked_config, "fallback must identify reviewed config"


@pytest.mark.parametrize(
    "document",
    [
        _dictionary_text().replace("schema = 1", "schema = 2"),
        _dictionary_text().replace('[oxford]\nstems = ["organ"]', 'oxford = "bad"'),
        _dictionary_text().replace('stems = ["organ"]', "stems = [1]"),
        _dictionary_text().replace(
            "[words.corrections]", "[words.corrections]\nteh = 1"
        ),
    ],
    ids=("schema", "table", "string-list", "correction"),
)
def test_dictionary_validation_rejects_invalid_documents(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
    document: str,
) -> None:
    """Schema, table, string-list and correction types remain validated."""
    _, rollout, _ = rollout_modules
    source = tmp_path / "base.toml"
    source.write_text(document, encoding="utf-8")
    with pytest.raises((TypeError, ValueError)):
        rollout.load_dictionary(source)


def test_merge_rejects_conflicting_corrections(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
) -> None:
    """A local overlay cannot silently weaken a shared correction."""
    _, rollout, _ = rollout_modules
    base = rollout.Dictionary(corrections=(("teh", "the"),))
    local = rollout.Dictionary(corrections=(("teh", "ten"),))

    with pytest.raises(ValueError, match="conflicting correction"):
        rollout.merge_dictionaries(base, local)


def test_render_and_write_are_deterministic_valid_toml(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Rendering is stable, parseable and atomically installed."""
    _, rollout, _ = rollout_modules
    dictionary = rollout.Dictionary(
        stems=("organ",),
        accepted=("proper-name",),
        ignore_patterns=("https?://",),
        excluded_files=("target",),
    )
    output = tmp_path / "nested" / "typos.toml"

    first = rollout.render_typos_config(dictionary)
    rollout.write_config(output, dictionary)

    assert first == rollout.render_typos_config(dictionary), "rendering must be stable"
    assert output.read_text(encoding="utf-8") == first, "write must install rendering"
    assert tomllib.loads(first)["default"]["locale"] == "en-gb", (
        "generated locale must remain en-gb"
    )
    assert list(output.parent.glob(".typos.toml.*")) == [], (
        "atomic writes must not leave temporary files"
    )


def test_offline_refresh_requires_and_reuses_valid_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Offline mode fails closed before reusing a validated cache."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "base.toml"
    metadata = tmp_path / "base.json"

    with pytest.raises(FileNotFoundError, match="no cached shared dictionary"):
        rollout.refresh_base(
            "https://example.invalid/base", cache, metadata=metadata, offline=True
        )

    cache.write_text(_dictionary_text(), encoding="utf-8")
    result = rollout.refresh_base(
        "https://example.invalid/base", cache, metadata=metadata, offline=True
    )

    assert result.status == "offline-cache", "offline mode must report cache reuse"


def test_local_refresh_switches_authority_and_records_metadata(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """A different explicit authority replaces a cache regardless of mtime."""
    _, rollout, _ = rollout_modules
    first = tmp_path / "first.toml"
    second = tmp_path / "second.toml"
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    first.write_text(_dictionary_text("first"), encoding="utf-8")
    second.write_text(_dictionary_text("second"), encoding="utf-8")
    os.utime(first, ns=(3_000_000_000, 3_000_000_000))
    os.utime(second, ns=(1_000_000_000, 1_000_000_000))
    rollout.refresh_base(first, cache, metadata=metadata)

    result = rollout.refresh_base(second, cache, metadata=metadata)

    assert result.status == "refreshed", "new authority must replace the cache"
    assert rollout.load_dictionary(cache).stems == ("second",), (
        "replacement authority must supply policy"
    )
    assert json.loads(metadata.read_text(encoding="utf-8"))["source"] == str(
        second.resolve()
    ), "metadata must record replacement authority"


def test_http_refresh_uses_validators_and_preserves_newer_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Remote refresh uses validators only with the source that supplied them."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    requests: list[urllib.request.Request] = []

    class Response:
        """Provide the HTTP response surface consumed by the helper."""

        status = 200
        headers: typ.ClassVar[dict[str, str]] = {
            "ETag": '"estate-v1"',
            "Last-Modified": "Fri, 10 Jul 2026 08:00:00 GMT",
        }

        def read(self) -> bytes:
            """Return a valid shared dictionary."""
            return _dictionary_text().encode()

        def __enter__(self) -> Response:
            """Enter the fake response context."""
            return self

        def __exit__(self, *_args: object) -> None:
            """Leave the fake response context."""

    def open_response(request: urllib.request.Request, *, timeout: float) -> Response:
        """Capture the request passed to the network boundary."""
        assert timeout == pytest.approx(30.0), "HTTP timeout must remain bounded"
        requests.append(request)
        return Response()

    first = rollout.refresh_base(
        "https://example.test/base.toml", cache, metadata=metadata, opener=open_response
    )
    second = rollout.refresh_base(
        "https://example.test/base.toml", cache, metadata=metadata, opener=open_response
    )
    replacement = rollout.refresh_base(
        "https://example.test/replacement.toml",
        cache,
        metadata=metadata,
        opener=open_response,
    )

    assert first.status == "refreshed", "first response must populate the cache"
    assert second.status == "current", "matching ETag must preserve the cache"
    assert requests[1].get_header("If-none-match") == '"estate-v1"', (
        "subsequent requests must send the saved ETag"
    )
    assert replacement.status == "refreshed", "new authority must be downloaded"
    assert requests[2].get_header("If-none-match") is None, (
        "validators must not cross authority boundaries"
    )
    assert requests[2].get_header("If-modified-since") is None, (
        "dates must not cross authority boundaries"
    )


def test_remote_failure_reuses_only_a_valid_stale_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Network failure keeps validated data and propagates without it."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"

    def fail(*_args: object, **_kwargs: object) -> None:
        """Model an unavailable remote authority."""
        message = "offline"
        raise urllib.error.URLError(message)

    with pytest.raises(rollout.NetworkUnavailableError):
        rollout.refresh_base(
            "https://example.test/base", cache, metadata=metadata, opener=fail
        )

    cache.write_text(_dictionary_text(), encoding="utf-8")
    result = rollout.refresh_base(
        "https://example.test/base", cache, metadata=metadata, opener=fail
    )

    assert result.status == "stale-cache", "connectivity failure may reuse valid data"


def test_remote_refresh_rejects_insecure_and_invalid_content(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """The remote boundary requires HTTPS and validates bytes before install."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"

    with pytest.raises(ValueError, match="must use HTTPS"):
        rollout.refresh_base("http://example.test/base", cache, metadata=metadata)

    class InvalidResponse:
        """Return malformed TOML from an otherwise successful response."""

        status = 200
        headers: typ.ClassVar[dict[str, str]] = {}

        def read(self) -> bytes:
            """Return malformed bytes."""
            return b"not = [valid"

        def __enter__(self) -> InvalidResponse:
            """Enter the fake response context."""
            return self

        def __exit__(self, *_args: object) -> None:
            """Leave the fake response context."""

    with pytest.raises(tomllib.TOMLDecodeError):
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            metadata=metadata,
            opener=lambda *_args, **_kwargs: InvalidResponse(),
        )
    assert not cache.exists(), "invalid remote content must not be installed"


def test_metadata_reader_handles_invalid_and_non_object_json(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Malformed or non-object freshness metadata is safely ignored."""
    _, rollout, _ = rollout_modules
    metadata = tmp_path / "cache.json"

    metadata.write_text("not-json", encoding="utf-8")
    assert rollout._read_metadata(metadata) == {}, "invalid JSON must be ignored"
    metadata.write_text("[]", encoding="utf-8")
    assert rollout._read_metadata(metadata) == {}, "non-object JSON must be ignored"
