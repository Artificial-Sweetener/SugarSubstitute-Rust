#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Make the bounded HTTPS requests the self-updater needs, and nothing else.

Every request is HTTPS (or plain HTTP to the loopback address) to an
allow-listed host, follows at most a few redirects and only to allowed hosts, has a per-read timeout and an overall deadline, and is
cut off at a byte limit. Failures are typed so the updater can say plainly
whether it could not reach the internet or the release was refused.
"""

from __future__ import annotations

import hashlib
import json
import socket
import ssl
import time
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from enum import Enum
from http.client import HTTPMessage
from pathlib import Path
from typing import IO, Protocol

import certifi

_USER_AGENT = "SugarCubes-self-update"
_READ_TIMEOUT_SECONDS = 20.0
_METADATA_DEADLINE_SECONDS = 60.0
_DOWNLOAD_DEADLINE_SECONDS = 600.0
_CHUNK_BYTES = 256 * 1024


class HttpFailureKind(str, Enum):
    """Name why one request did not produce a usable response."""

    UNREACHABLE = "unreachable"
    TIMED_OUT = "timed-out"
    NOT_FOUND = "not-found"
    REFUSED = "refused"
    TOO_LARGE = "too-large"
    UNTRUSTED = "untrusted"
    INVALID = "invalid"
    CANCELLED = "cancelled"


class HttpFailure(RuntimeError):
    """Report one failed request with a typed kind and no secrets in its text."""

    def __init__(self, kind: HttpFailureKind, detail: str) -> None:
        """Keep the kind beside a short diagnostic."""

        super().__init__(f"{kind.value}: {detail}")
        self.kind = kind
        self.detail = detail


Cancelled = Callable[[], bool]


class HttpClient(Protocol):
    """Fetch release metadata and archives within fixed bounds."""

    def get_json(
        self, url: str, *, allowed_hosts: frozenset[str], max_bytes: int
    ) -> object:
        """Return the decoded JSON body of one GET request."""

    def download(
        self,
        url: str,
        target: Path,
        *,
        allowed_hosts: frozenset[str],
        max_bytes: int,
        cancelled: Cancelled,
    ) -> str:
        """Write one response body to `target` and return its SHA-256."""


LOOPBACK_HOSTS = frozenset({"127.0.0.1", "::1", "localhost"})
MAX_REDIRECTS = 5


def origin_is_allowed(scheme: str, host: str | None, port: int | None) -> bool:
    """Return whether an origin may be contacted at all.

    HTTPS on its standard port anywhere, or plain HTTP to this computer's own
    loopback address, which nothing on the network can see or alter.
    """

    if host in LOOPBACK_HOSTS:
        return scheme in {"https", "http"}
    return scheme == "https" and port in {None, 443}


def require_allowed_url(url: str, allowed_hosts: frozenset[str]) -> None:
    """Reject any URL that is not an allowed origin on an allow-listed host."""

    parsed = urllib.parse.urlsplit(url)
    try:
        port = parsed.port
    except ValueError as exc:
        raise HttpFailure(
            HttpFailureKind.UNTRUSTED, "The address has a bad port."
        ) from exc
    if (
        not origin_is_allowed(parsed.scheme, parsed.hostname, port)
        or parsed.hostname not in allowed_hosts
        or parsed.username is not None
        or parsed.password is not None
        or parsed.fragment
    ):
        raise HttpFailure(
            HttpFailureKind.UNTRUSTED, f"Refused an address on {parsed.hostname!r}."
        )


class _AllowListRedirects(urllib.request.HTTPRedirectHandler):
    """Follow a redirect only to an allow-listed host, and only a few times."""

    max_redirections = MAX_REDIRECTS

    def __init__(self, allowed_hosts: frozenset[str]) -> None:
        """Bind the redirect policy to one request's host list."""

        super().__init__()
        self._allowed_hosts = allowed_hosts

    def redirect_request(
        self,
        req: urllib.request.Request,
        fp: IO[bytes],
        code: int,
        msg: str,
        headers: HTTPMessage,
        newurl: str,
    ) -> urllib.request.Request | None:
        """Validate the redirect target before urllib follows it."""

        require_allowed_url(newurl, self._allowed_hosts)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


@dataclass(frozen=True, slots=True)
class UrllibHttpClient:
    """Implement the bounded client over urllib with certifi's trust store."""

    read_timeout_seconds: float = _READ_TIMEOUT_SECONDS
    metadata_deadline_seconds: float = _METADATA_DEADLINE_SECONDS
    download_deadline_seconds: float = _DOWNLOAD_DEADLINE_SECONDS
    # The overall deadline is measured on this clock, so tests can drive it.
    clock: Callable[[], float] = time.monotonic

    def get_json(
        self, url: str, *, allowed_hosts: frozenset[str], max_bytes: int
    ) -> object:
        """Return the decoded JSON body of one bounded GET request."""

        chunks: list[bytes] = []
        self._stream(
            url,
            allowed_hosts=allowed_hosts,
            max_bytes=max_bytes,
            accept="application/json",
            deadline=self.metadata_deadline_seconds,
            cancelled=lambda: False,
            sink=chunks.append,
        )
        try:
            return json.loads(b"".join(chunks).decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise HttpFailure(
                HttpFailureKind.INVALID, "The reply is not JSON."
            ) from exc

    def download(
        self,
        url: str,
        target: Path,
        *,
        allowed_hosts: frozenset[str],
        max_bytes: int,
        cancelled: Cancelled,
    ) -> str:
        """Stream one archive to `target` and return its SHA-256."""

        digest = hashlib.sha256()
        with target.open("xb") as output:

            def sink(chunk: bytes) -> None:
                digest.update(chunk)
                output.write(chunk)

            self._stream(
                url,
                allowed_hosts=allowed_hosts,
                max_bytes=max_bytes,
                accept="application/octet-stream",
                deadline=self.download_deadline_seconds,
                cancelled=cancelled,
                sink=sink,
            )
        return digest.hexdigest()

    def _stream(
        self,
        url: str,
        *,
        allowed_hosts: frozenset[str],
        max_bytes: int,
        accept: str,
        deadline: float,
        cancelled: Cancelled,
        sink: Callable[[bytes], None],
    ) -> None:
        """Read one response in chunks, enforcing the size limit and deadline."""

        require_allowed_url(url, allowed_hosts)
        opener = urllib.request.build_opener(
            urllib.request.HTTPSHandler(
                context=ssl.create_default_context(cafile=certifi.where())
            ),
            _AllowListRedirects(allowed_hosts),
        )
        request = urllib.request.Request(
            url, headers={"Accept": accept, "User-Agent": _USER_AGENT}
        )
        expires_at = self.clock() + deadline
        received = 0
        try:
            with opener.open(request, timeout=self.read_timeout_seconds) as response:
                # read1 returns what has arrived, so a slow drip cannot outlast the deadline.
                while chunk := response.read1(_CHUNK_BYTES):
                    received += len(chunk)
                    if received > max_bytes:
                        raise HttpFailure(
                            HttpFailureKind.TOO_LARGE, "The reply is too large."
                        )
                    if cancelled():
                        raise HttpFailure(
                            HttpFailureKind.CANCELLED, "The request was stopped."
                        )
                    if self.clock() > expires_at:
                        raise HttpFailure(
                            HttpFailureKind.TIMED_OUT, "The reply took too long."
                        )
                    sink(chunk)
        except urllib.error.HTTPError as exc:
            kind = (
                HttpFailureKind.NOT_FOUND
                if exc.code == 404
                else HttpFailureKind.REFUSED
            )
            raise HttpFailure(kind, f"The server answered {exc.code}.") from exc
        except (TimeoutError, socket.timeout) as exc:
            raise HttpFailure(
                HttpFailureKind.TIMED_OUT, "The server did not answer."
            ) from exc
        except urllib.error.URLError as exc:
            if isinstance(exc.reason, HttpFailure):
                raise exc.reason from exc
            raise HttpFailure(
                HttpFailureKind.UNREACHABLE,
                f"No connection ({type(exc.reason).__name__}).",
            ) from exc
        except OSError as exc:
            raise HttpFailure(
                HttpFailureKind.UNREACHABLE, f"No connection ({type(exc).__name__})."
            ) from exc


__all__ = [
    "Cancelled",
    "HttpClient",
    "HttpFailure",
    "HttpFailureKind",
    "LOOPBACK_HOSTS",
    "MAX_REDIRECTS",
    "UrllibHttpClient",
    "origin_is_allowed",
    "require_allowed_url",
]
