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
"""Prove the real urllib transport's bounds against a loopback HTTP server."""

from __future__ import annotations

import dataclasses
import hashlib
import itertools
import threading
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import pytest

from sugarcubes.backend.self_update.http_client import (
    MAX_REDIRECTS,
    HttpFailure,
    HttpFailureKind,
    UrllibHttpClient,
)
from tests.support.no_progress import join_thread

PAYLOAD = b"x" * 4096
DRIP_LENGTH = 64 * 1024 * 1024
HOSTS = frozenset({"127.0.0.1"})


class _Server(ThreadingHTTPServer):
    """Hold the release event that ends every stalled response."""

    daemon_threads = True
    release: threading.Event


class _Handler(BaseHTTPRequestHandler):
    """Serve the fixed responses each transport bound is tested against."""

    server: _Server

    def log_message(self, format: str, *args: object) -> None:
        """Keep test output quiet."""

    def _send(self, body: bytes, *, length: int | None = None) -> None:
        self.send_response(200)
        self.send_header("Content-Length", str(len(body) if length is None else length))
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def do_GET(self) -> None:
        """Answer one request by path."""

        path = self.path
        if path == "/json":
            self._send(b'{"ok": true}')
        elif path == "/file":
            self._send(PAYLOAD)
        elif path == "/idle":
            self.server.release.wait()
        elif path == "/stall":
            self._send(PAYLOAD[:1024], length=len(PAYLOAD))
            self.server.release.wait()
        elif path == "/drip":
            self.send_response(200)
            self.send_header("Content-Length", str(DRIP_LENGTH))
            self.end_headers()
            while not self.server.release.is_set():
                self.wfile.write(b"x" * 64)
                self.wfile.flush()
        elif path.startswith("/hop/"):
            remaining = int(path.removeprefix("/hop/"))
            self.send_response(302)
            self.send_header(
                "Location", "/file" if remaining == 0 else f"/hop/{remaining - 1}"
            )
            self.send_header("Content-Length", "0")
            self.end_headers()
        elif path == "/elsewhere":
            self.send_response(302)
            self.send_header("Location", "http://127.0.0.2/file")
            self.send_header("Content-Length", "0")
            self.end_headers()
        else:
            self.send_error(404)


@pytest.fixture
def server() -> Iterator[str]:
    """Run one loopback server for a test and stop it afterwards."""

    instance = _Server(("127.0.0.1", 0), _Handler)
    instance.release = threading.Event()
    thread = threading.Thread(target=instance.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{instance.server_address[1]}"
    finally:
        instance.release.set()
        instance.shutdown()
        instance.server_close()
        join_thread(thread)


def _client() -> UrllibHttpClient:
    return UrllibHttpClient(
        read_timeout_seconds=0.5,
        metadata_deadline_seconds=2.0,
        download_deadline_seconds=1.0,
    )


def test_json_and_downloads_succeed_within_bounds(server: str, tmp_path: Path) -> None:
    """A normal reply is decoded, and a download returns the file's digest."""

    client = _client()
    assert client.get_json(f"{server}/json", allowed_hosts=HOSTS, max_bytes=100) == {
        "ok": True
    }
    target = tmp_path / "file.bin"
    digest = client.download(
        f"{server}/file",
        target,
        allowed_hosts=HOSTS,
        max_bytes=10_000,
        cancelled=lambda: False,
    )
    assert digest == hashlib.sha256(PAYLOAD).hexdigest()
    assert target.read_bytes() == PAYLOAD


def test_reply_over_the_size_limit_is_cut_off(server: str, tmp_path: Path) -> None:
    """The byte limit is enforced while reading, not trusted from headers."""

    with pytest.raises(HttpFailure) as raised:
        _client().download(
            f"{server}/file",
            tmp_path / "f",
            allowed_hosts=HOSTS,
            max_bytes=1000,
            cancelled=lambda: False,
        )
    assert raised.value.kind is HttpFailureKind.TOO_LARGE


def test_server_that_never_answers_times_out(server: str) -> None:
    """An idle server ends the request at the read timeout."""

    with pytest.raises(HttpFailure) as raised:
        _client().get_json(f"{server}/idle", allowed_hosts=HOSTS, max_bytes=100)
    assert raised.value.kind is HttpFailureKind.TIMED_OUT


def test_server_that_stalls_mid_body_times_out(server: str, tmp_path: Path) -> None:
    """A body that stops arriving part way ends at the read timeout."""

    with pytest.raises(HttpFailure) as raised:
        _client().download(
            f"{server}/stall",
            tmp_path / "f",
            allowed_hosts=HOSTS,
            max_bytes=10_000,
            cancelled=lambda: False,
        )
    assert raised.value.kind is HttpFailureKind.TIMED_OUT


def test_body_still_arriving_at_the_deadline_is_ended(
    server: str, tmp_path: Path
) -> None:
    """Data arriving faster than the read timeout cannot outlast the overall deadline.

    The deadline runs on a clock that advances one second per reading, so the
    outcome depends on how many chunks were read, never on elapsed time.
    """

    ticks = itertools.count()
    client = dataclasses.replace(
        _client(), download_deadline_seconds=5.0, clock=lambda: float(next(ticks))
    )
    with pytest.raises(HttpFailure) as raised:
        client.download(
            f"{server}/drip",
            tmp_path / "f",
            allowed_hosts=HOSTS,
            max_bytes=DRIP_LENGTH,
            cancelled=lambda: False,
        )
    assert raised.value.kind is HttpFailureKind.TIMED_OUT
    assert (tmp_path / "f").stat().st_size < DRIP_LENGTH


def test_cancellation_stops_a_download(server: str, tmp_path: Path) -> None:
    """Closing the updater stops a running download at the next chunk."""

    with pytest.raises(HttpFailure) as raised:
        _client().download(
            f"{server}/drip",
            tmp_path / "f",
            allowed_hosts=HOSTS,
            max_bytes=DRIP_LENGTH,
            cancelled=lambda: True,
        )
    assert raised.value.kind is HttpFailureKind.CANCELLED


def test_redirects_are_followed_only_up_to_the_limit(
    server: str, tmp_path: Path
) -> None:
    """A short redirect chain works; a longer one is refused."""

    client = _client()
    client.download(
        f"{server}/hop/{MAX_REDIRECTS - 1}",
        tmp_path / "ok",
        allowed_hosts=HOSTS,
        max_bytes=10_000,
        cancelled=lambda: False,
    )
    with pytest.raises(HttpFailure) as raised:
        client.download(
            f"{server}/hop/{MAX_REDIRECTS + 2}",
            tmp_path / "too-many",
            allowed_hosts=HOSTS,
            max_bytes=10_000,
            cancelled=lambda: False,
        )
    assert raised.value.kind is HttpFailureKind.REFUSED


def test_redirect_off_the_allowed_hosts_is_refused(server: str, tmp_path: Path) -> None:
    """A redirect cannot carry the request to a host that is not allowed."""

    with pytest.raises(HttpFailure) as raised:
        _client().download(
            f"{server}/elsewhere",
            tmp_path / "f",
            allowed_hosts=HOSTS,
            max_bytes=10_000,
            cancelled=lambda: False,
        )
    assert raised.value.kind is HttpFailureKind.UNTRUSTED


def test_plain_http_is_refused_off_loopback() -> None:
    """Only this computer's own address may be reached without TLS."""

    with pytest.raises(HttpFailure) as raised:
        _client().get_json(
            "http://example.com/json",
            allowed_hosts=frozenset({"example.com"}),
            max_bytes=100,
        )
    assert raised.value.kind is HttpFailureKind.UNTRUSTED
