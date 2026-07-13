"""Provide cache and HTTPS boundaries for the spelling-policy helper.

The module owns refresh result types, guarded HTTP request construction, and
cleanup-safe atomic writes used by :mod:`typos_rollout`.
"""

from __future__ import annotations

import dataclasses as dc
import pathlib
import tempfile
import typing as typ
import urllib.parse
import urllib.request

if typ.TYPE_CHECKING:
    import collections.abc as cabc


@dc.dataclass(frozen=True)
class RefreshResult:
    """Describe whether the untracked shared dictionary cache changed.

    Attributes
    ----------
    status
        Stable refresh outcome such as ``refreshed`` or ``current``.
    cache
        Path to the validated untracked base cache.
    """

    status: str
    cache: pathlib.Path


@dc.dataclass(frozen=True)
class CacheTargets:
    """Group the untracked cache and metadata sidecar destinations.

    Attributes
    ----------
    cache
        Path receiving validated dictionary bytes.
    metadata
        Path receiving source identity and freshness validators.
    """

    cache: pathlib.Path
    metadata: pathlib.Path


class NetworkUnavailableError(OSError):
    """Report that the remote dictionary authority could not be reached."""


class InsecureSourceError(ValueError):
    """Report a dictionary source or redirect that does not use HTTPS."""


class RemoteResponse(typ.Protocol):
    """Expose the response surface consumed by remote cache refreshes.

    Attributes
    ----------
    status
        HTTP response status.
    headers
        Response headers containing optional freshness validators.
    """

    status: int
    headers: cabc.Mapping[str, str]

    def read(self) -> bytes:
        """Read the complete response body.

        Returns
        -------
        bytes
            Downloaded dictionary content.
        """
        ...

    def __enter__(self) -> RemoteResponse:
        """Enter the managed response context.

        Returns
        -------
        RemoteResponse
            Open response object.
        """
        ...

    def __exit__(self, *args: object) -> None:
        """Leave the managed response context.

        Parameters
        ----------
        *args
            Optional exception context supplied by the context manager.
        """
        ...


class HttpsRedirectHandler(urllib.request.HTTPRedirectHandler):
    """Reject redirects that leave the HTTPS transport boundary."""

    def redirect_request(
        self,
        request: urllib.request.Request,
        file_pointer: object,
        code: int,
        message: str,
        headers: object,
        new_url: str,
    ) -> urllib.request.Request | None:
        """Follow only redirects whose resolved target remains HTTPS."""
        if urllib.parse.urlsplit(new_url).scheme != "https":
            error_message = f"shared dictionary redirect must use HTTPS: {new_url}"
            raise InsecureSourceError(error_message)
        return super().redirect_request(
            request,
            file_pointer,
            code,
            message,
            headers,
            new_url,
        )


HTTPS_OPENER = urllib.request.build_opener(HttpsRedirectHandler())


def https_request(
    source: str,
    headers: cabc.Mapping[str, str],
) -> urllib.request.Request:
    """Build a request after constraining the shared source to HTTPS.

    Parameters
    ----------
    source
        Remote shared-dictionary URL.
    headers
        Conditional request headers for the current source.

    Returns
    -------
    urllib.request.Request
        Request accepted by the guarded HTTPS opener.

    Raises
    ------
    InsecureSourceError
        If ``source`` does not use HTTPS.

    Examples
    --------
    Build a conditional HTTPS request::

        https_request("https://example.test/base.toml", {"If-None-Match": '"v1"'})
    """
    if urllib.parse.urlsplit(source).scheme != "https":
        message = f"shared dictionary URL must use HTTPS: {source}"
        raise InsecureSourceError(message)
    return urllib.request.Request(source, headers=dict(headers))


def atomic_write(path: pathlib.Path, content: bytes) -> None:
    """Atomically replace a destination and clean temporary files.

    Parameters
    ----------
    path
        Destination path to replace.
    content
        Complete bytes to persist.

    Returns
    -------
    None
        The destination is the function's only result.

    Raises
    ------
    OSError
        If directory creation, writing, closing, or replacement fails.

    Examples
    --------
    Persist generated configuration without exposing a partial file::

        atomic_write(pathlib.Path("typos.toml"), b"[default]\n")
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    stream = tempfile.NamedTemporaryFile(
        delete=False,
        dir=path.parent,
        prefix=f".{path.name}.",
    )
    temporary = pathlib.Path(stream.name)
    try:
        with stream:
            stream.write(content)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)
