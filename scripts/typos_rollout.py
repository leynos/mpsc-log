"""Refresh cached shared en-GB-oxendict dictionary policy.

The module coordinates local and HTTPS authorities, validator metadata,
connectivity-only stale fallback, and atomic persistence before delegating pure
dictionary validation and rendering to :mod:`typos_rollout_dictionary`.
"""

from __future__ import annotations

import email.utils
import json
import pathlib
import tomllib
import typing as typ
import urllib.error

import typos_rollout_cache
import typos_rollout_dictionary

if typ.TYPE_CHECKING:
    import collections.abc as cabc

RefreshResult = typos_rollout_cache.RefreshResult
NetworkUnavailableError = typos_rollout_cache.NetworkUnavailableError
InsecureSourceError = typos_rollout_cache.InsecureSourceError
_CacheTargets = typos_rollout_cache.CacheTargets
_RemoteResponse = typos_rollout_cache.RemoteResponse
_atomic_write = typos_rollout_cache.atomic_write
HTTP_NOT_MODIFIED = 304

Dictionary = typos_rollout_dictionary.Dictionary
SUFFIX_PAIRS = typos_rollout_dictionary.SUFFIX_PAIRS
_dictionary_from_text = typos_rollout_dictionary._dictionary_from_text
load_dictionary = typos_rollout_dictionary.load_dictionary
merge_dictionaries = typos_rollout_dictionary.merge_dictionaries
generate_word_mappings = typos_rollout_dictionary.generate_word_mappings
render_typos_config = typos_rollout_dictionary.render_typos_config


def write_config(path: pathlib.Path, dictionary: Dictionary) -> None:
    """Atomically write validated generated configuration.

    Parameters
    ----------
    path
        Destination for the generated ``typos`` configuration.
    dictionary
        Validated policy to render and persist.

    Examples
    --------
    Write a minimal generated configuration::

        write_config(pathlib.Path("typos.toml"), Dictionary(stems=("organ",)))
    """
    _atomic_write(path, render_typos_config(dictionary).encode())


def _read_metadata(path: pathlib.Path) -> dict[str, object]:
    """Read best-effort HTTP freshness metadata."""
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def _write_metadata(
    path: pathlib.Path,
    metadata: cabc.Mapping[str, object],
) -> None:
    """Atomically write HTTP freshness metadata."""
    _atomic_write(path, (json.dumps(metadata, sort_keys=True) + "\n").encode())


def _valid_cache(cache: pathlib.Path) -> bool:
    """Return whether *cache* contains a valid shared dictionary."""
    try:
        load_dictionary(cache)
    except (
        FileNotFoundError,
        OSError,
        TypeError,
        ValueError,
        tomllib.TOMLDecodeError,
    ):
        return False
    return True


def _remote_is_not_newer(
    saved: cabc.Mapping[str, object],
    headers: cabc.Mapping[str, str],
) -> bool:
    """Return whether HTTP validators prove the response is not newer."""
    etag = headers.get("ETag")
    saved_etag = saved.get("etag")
    if isinstance(etag, str) and isinstance(saved_etag, str):
        return etag == saved_etag
    modified = headers.get("Last-Modified")
    saved_modified = saved.get("last_modified")
    if not isinstance(modified, str) or not isinstance(saved_modified, str):
        return False
    try:
        return email.utils.parsedate_to_datetime(
            modified
        ) <= email.utils.parsedate_to_datetime(saved_modified)
    except (TypeError, ValueError):
        return modified == saved_modified


def _local_cache_is_current(
    cache: pathlib.Path,
    saved: cabc.Mapping[str, object],
    source_name: str,
    source_mtime_ns: int,
) -> bool:
    """Return whether metadata proves a valid local-source cache is current."""
    saved_mtime = saved.get("mtime_ns")
    has_matching_source = saved.get("source") == source_name
    has_new_enough_mtime = (
        isinstance(saved_mtime, int) and source_mtime_ns <= saved_mtime
    )
    return _valid_cache(cache) and has_matching_source and has_new_enough_mtime


def _refresh_local(
    source: pathlib.Path,
    cache: pathlib.Path,
    metadata: pathlib.Path,
) -> RefreshResult:
    """Refresh from a local authoritative copy when it is newer."""
    source_stat = source.stat()
    source_name = str(source.resolve())
    saved = _read_metadata(metadata)
    if _local_cache_is_current(
        cache,
        saved,
        source_name,
        source_stat.st_mtime_ns,
    ):
        return RefreshResult("current", cache)
    content = source.read_bytes()
    _dictionary_from_text(content.decode())
    _atomic_write(cache, content)
    _write_metadata(
        metadata,
        {"source": source_name, "mtime_ns": source_stat.st_mtime_ns},
    )
    return RefreshResult("refreshed", cache)


def _conditional_headers(saved: cabc.Mapping[str, object]) -> dict[str, str]:
    """Build conditional HTTP headers from persisted validators."""
    headers: dict[str, str] = {}
    etag = saved.get("etag")
    if isinstance(etag, str):
        headers["If-None-Match"] = etag
    last_modified = saved.get("last_modified")
    if isinstance(last_modified, str):
        headers["If-Modified-Since"] = last_modified
    return headers


def _write_remote_cache(
    source: str,
    targets: _CacheTargets,
    response: _RemoteResponse,
) -> RefreshResult:
    """Validate and atomically persist an HTTP dictionary response."""
    try:
        content = response.read()
    except urllib.error.URLError as error:
        message = f"shared dictionary authority is unavailable: {source}"
        raise NetworkUnavailableError(message) from error
    _dictionary_from_text(content.decode())
    _atomic_write(targets.cache, content)
    _write_metadata(
        targets.metadata,
        {
            "source": source,
            "etag": response.headers.get("ETag"),
            "last_modified": response.headers.get("Last-Modified"),
        },
    )
    return RefreshResult("refreshed", targets.cache)


def _remote_response_result(
    source: str,
    targets: _CacheTargets,
    saved: cabc.Mapping[str, object],
    response: _RemoteResponse,
) -> RefreshResult:
    """Return the cache result for a successful HTTP response."""
    if _valid_cache(targets.cache) and _remote_is_not_newer(saved, response.headers):
        return RefreshResult("current", targets.cache)
    return _write_remote_cache(source, targets, response)


def _stale_cache_or_raise(
    cache: pathlib.Path,
    error: NetworkUnavailableError,
) -> RefreshResult:
    """Return a valid stale cache or propagate the download failure."""
    if _valid_cache(cache):
        return RefreshResult("stale-cache", cache)
    raise error


def _http_error_result(
    cache: pathlib.Path,
    error: urllib.error.HTTPError,
) -> RefreshResult:
    """Translate an HTTP failure into the available cache result."""
    if error.code == HTTP_NOT_MODIFIED and _valid_cache(cache):
        return RefreshResult("current", cache)
    raise error


def _refresh_http(
    source: str,
    cache: pathlib.Path,
    metadata: pathlib.Path,
    opener: cabc.Callable[..., _RemoteResponse] | None,
) -> RefreshResult:
    """Refresh a cache from a validated HTTPS source with stale fallback."""
    saved = _read_metadata(metadata)
    if saved.get("source") != source:
        saved = {}
    request = typos_rollout_cache.https_request(source, _conditional_headers(saved))
    open_remote = typos_rollout_cache.HTTPS_OPENER.open if opener is None else opener
    try:
        response_context = open_remote(request, timeout=30.0)
    except urllib.error.HTTPError as error:
        return _http_error_result(cache, error)
    except urllib.error.URLError:
        message = f"shared dictionary authority is unavailable: {source}"
        return _stale_cache_or_raise(cache, NetworkUnavailableError(message))
    with response_context as response:
        try:
            return _remote_response_result(
                source, _CacheTargets(cache, metadata), saved, response
            )
        except NetworkUnavailableError as error:
            return _stale_cache_or_raise(cache, error)


def refresh_base(
    source: str | pathlib.Path,
    cache: pathlib.Path,
    *,
    metadata: pathlib.Path,
    offline: bool = False,
    opener: cabc.Callable[..., _RemoteResponse] | None = None,
) -> RefreshResult:
    """Refresh an untracked base cache when the authority is newer.

    Parameters
    ----------
    source
        Local path or HTTPS URL for the shared authority.
    cache
        Untracked destination for validated shared policy.
    metadata
        Sidecar recording source identity and freshness validators.
    offline
        Reuse only a valid local cache when true.
    opener
        Optional injectable HTTPS boundary for tests.

    Returns
    -------
    RefreshResult
        Cache path and stable refresh outcome.

    Examples
    --------
    Refresh from a checked-out shared policy::

        refresh_base(
            pathlib.Path("shared.toml"),
            pathlib.Path(".typos-base.toml"),
            metadata=pathlib.Path(".typos-base.json"),
        )
    """
    if offline:
        if not _valid_cache(cache):
            message = f"no cached shared dictionary at {cache}"
            raise FileNotFoundError(message)
        return RefreshResult("offline-cache", cache)
    if isinstance(source, pathlib.Path) or "://" not in str(source):
        return _refresh_local(pathlib.Path(source), cache, metadata)
    return _refresh_http(str(source), cache, metadata, opener)
