"""Exercise security and integration contracts raised during rollout review."""

from __future__ import annotations

import email.message
import pathlib
import typing as typ
import urllib.error

import pytest

if typ.TYPE_CHECKING:
    import types


def _dictionary_text(*, accepted: str = "") -> str:
    """Return a minimal shared dictionary document."""
    words = f'"{accepted}"' if accepted else ""
    return (
        "schema = 1\n\n[oxford]\nstems = []\n\n[words]\n"
        f"accepted = [{words}]\n\n[words.corrections]\n\n"
        "[patterns]\nignore = []\n\n[files]\nexclude = []\n"
    )


def test_generator_merges_local_overlay_on_success(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: pathlib.Path,
) -> None:
    """Successful generation combines shared policy with a local proper name."""
    _, _, generator = rollout_modules
    source = tmp_path / "shared.toml"
    source.write_text(_dictionary_text(accepted="SharedName"), encoding="utf-8")
    (tmp_path / "typos.local.toml").write_text(
        _dictionary_text(accepted="LocalName"), encoding="utf-8"
    )

    result = generator.main(repository=tmp_path, source=source)
    generated = (tmp_path / "typos.toml").read_text(encoding="utf-8")

    assert result.status == "refreshed", "local authority must populate the cache"
    assert '"SharedName" = "SharedName"' in generated, "shared words must remain"
    assert '"LocalName" = "LocalName"' in generated, "local words must be merged"


def test_http_errors_never_use_stale_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: pathlib.Path,
) -> None:
    """Only 304 is a successful HTTP cache result; 5xx remains an error."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    cache.write_text(_dictionary_text(), encoding="utf-8")
    headers = email.message.Message()
    not_modified = urllib.error.HTTPError(
        "https://example.test", 304, "", headers, None
    )
    unavailable = urllib.error.HTTPError("https://example.test", 503, "", headers, None)

    assert rollout._http_error_result(cache, not_modified).status == "current", (
        "304 must retain a validated cache"
    )
    with pytest.raises(urllib.error.HTTPError):
        rollout._http_error_result(cache, unavailable)


def test_etag_takes_precedence_over_last_modified(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
) -> None:
    """A changed ETag cannot be hidden by an unchanged modification date."""
    _, rollout, _ = rollout_modules
    date = "Fri, 10 Jul 2026 08:00:00 GMT"

    assert not rollout._remote_is_not_newer(
        {"etag": '"old"', "last_modified": date},
        {"ETag": '"new"', "Last-Modified": date},
    ), "changed ETag must force refresh"
    assert rollout._remote_is_not_newer(
        {"last_modified": date}, {"Last-Modified": date}
    ), "date remains the fallback when ETags are unavailable"


def test_https_redirect_handler_rejects_insecure_target(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
) -> None:
    """The guarded opener rejects redirects that downgrade to HTTP."""
    cache_module, _, _ = rollout_modules
    handler = cache_module.HttpsRedirectHandler()

    with pytest.raises(cache_module.InsecureSourceError, match="redirect"):
        handler.redirect_request(
            object(), object(), 302, "redirect", object(), "http://example.test/base"
        )


def test_persistence_failure_is_not_treated_as_connectivity_failure(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
) -> None:
    """Local write errors propagate even when a valid response was received."""
    _, rollout, _ = rollout_modules

    class Response:
        """Return valid policy from the injectable network boundary."""

        status = 200
        headers: typ.ClassVar[dict[str, str]] = {}

        def read(self) -> bytes:
            """Return a valid response body."""
            return _dictionary_text().encode()

        def __enter__(self) -> Response:
            """Enter the response context."""
            return self

        def __exit__(self, *_args: object) -> None:
            """Leave the response context."""

    def deny_write(*_args: object, **_kwargs: object) -> None:
        """Model a local persistence failure."""
        raise PermissionError("read-only checkout")

    monkeypatch.setattr(rollout, "_atomic_write", deny_write)
    with pytest.raises(PermissionError, match="read-only"):
        rollout.refresh_base(
            "https://example.test/base",
            tmp_path / "cache.toml",
            metadata=tmp_path / "cache.json",
            opener=lambda *_args, **_kwargs: Response(),
        )


def test_atomic_write_removes_temporary_file_after_replace_failure(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
) -> None:
    """Atomic persistence removes its temporary file on installation failure."""
    cache_module, _, _ = rollout_modules
    destination = tmp_path / "typos.toml"

    def fail_replace(_path: pathlib.Path, _target: pathlib.Path) -> None:
        """Model an atomic replacement failure."""
        raise PermissionError("replace denied")

    monkeypatch.setattr(pathlib.Path, "replace", fail_replace)
    with pytest.raises(PermissionError, match="replace denied"):
        cache_module.atomic_write(destination, b"policy")

    assert list(tmp_path.glob(".typos.toml.*")) == [], (
        "failed replacement must not leave a temporary file"
    )
