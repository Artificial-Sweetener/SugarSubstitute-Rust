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
"""Prove bounded download redirects, credential isolation, cancellation, and public DNS policy."""

from __future__ import annotations


from tests.model_library.support.download_access import (
    fixture_download_authorization,
    fixture_offer,
)
import asyncio
from collections.abc import AsyncIterator
from dataclasses import replace
from pathlib import Path
import socket
from threading import Event

import aiohttp
from aiohttp.abc import ResolveResult
import pytest

from sugarcubes.model_library.infrastructure.artifact_downloads import (
    ModelArtifactDownloads,
    PublicModelDownloadResolver,
)
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.provider_transport import (
    TransportBody,
    TransportResponse,
    TransportSession,
)
from sugarcubes.model_library.infrastructure.artifact_files import ModelArtifactFile
from tests.support.no_progress import wait_for_event


class RedirectSession(TransportSession):
    """Return one reviewed cross-origin redirect followed by a tiny fixture artifact."""

    def get(
        self, url: str, *, allow_redirects: bool, headers: dict[str, str] | None = None
    ) -> TransportResponse:
        """Model only the external HTTP response boundary while preserving header observation."""
        self.response = (
            TransportResponse(302, b"")
            if not self.requests
            else TransportResponse(200, b"fixture", "application/octet-stream")
        )
        self.response.headers["Location"] = (
            "https://civitai-delivery-worker-prod.fixture.r2.cloudflarestorage.com/file"
        )
        return super().get(url, allow_redirects=allow_redirects, headers=headers)


def test_verified_download_strips_credentials_on_redirect(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Exercise the complete installer while replacing only remote HTTP transport."""
    session = RedirectSession(TransportResponse(200, b"fixture"), expected_timeout=7200)
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Install a tiny verified file through the production network and disk orchestration."""
        downloads = ModelArtifactDownloads(
            FixtureFileRoots(tmp_path),
            lambda: "fixture-token",
            fixture_download_authorization(),
        )
        assert await downloads.install(fixture_offer()) == "fixture.safetensors"

    asyncio.run(exercise())
    assert (tmp_path / "fixture.safetensors").read_bytes() == b"fixture"
    request_headers = session.headers[1:]
    assert request_headers[0]["Authorization"] == "Bearer fixture-token"
    assert "Authorization" not in request_headers[1]
    assert all(not redirected for _, redirected in session.requests)
    assert not tuple(tmp_path.glob("*.download"))


class OpenModelDbRedirectSession(TransportSession):
    """Model a reviewed direct origin redirecting to public artifact delivery."""

    def __init__(self, target: str) -> None:
        """Choose the external redirect without changing installer behavior."""
        super().__init__(TransportResponse(200, b"fixture"), expected_timeout=7200)
        self.target = target

    def get(
        self, url: str, *, allow_redirects: bool, headers: dict[str, str] | None = None
    ) -> TransportResponse:
        """Serve a redirect and then exact fixture bytes at the transport boundary."""
        self.response = (
            TransportResponse(302, b"")
            if not self.requests
            else TransportResponse(200, b"fixture")
        )
        self.response.headers["Location"] = self.target
        return super().get(url, allow_redirects=allow_redirects, headers=headers)


@pytest.mark.parametrize(
    "target",
    [
        "https://release-assets.githubusercontent.com/fixture.pth",
        "https://civitai.com/api/download/models/20",
    ],
)
def test_openmodeldb_download_uses_shared_verification_and_never_civitai_credentials(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, target: str
) -> None:
    """Publish tiny exact bytes through provider redirects without leaking another provider's token."""
    session = OpenModelDbRedirectSession(target)
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    offer = replace(
        fixture_offer(),
        file_name="fixture.pth",
        kind="upscale_models",
        provider="openmodeldb",
        download_url="https://github.com/example/releases/download/v1/fixture.pth",
    )
    progress: list[tuple[int, int]] = []

    async def exercise() -> None:
        """Observe hash verification and exclusive publication through the real installer."""
        installer = ModelArtifactDownloads(
            FixtureFileRoots(tmp_path, "upscale_models"),
            lambda: "private-civitai-token",
            fixture_download_authorization(),
        )
        assert (
            await installer.install(
                offer, lambda received, total: progress.append((received, total))
            )
            == "fixture.pth"
        )
        assert await installer.install(offer) == "fixture.pth"

    asyncio.run(exercise())
    assert (tmp_path / "fixture.pth").read_bytes() == b"fixture"
    assert progress == [(7, 7)]
    assert len(session.requests) == 2
    assert all("Authorization" not in headers for headers in session.headers)
    assert not tuple(tmp_path.glob("*.download"))


def test_existing_exact_artifact_is_reused_without_download(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Reuse only unchanged matching size and SHA256 under a legal model filename."""
    (tmp_path / "renamed.safetensors").write_bytes(b"fixture")
    (tmp_path / "wrong.safetensors").write_bytes(b"wrong!!")
    (tmp_path / "ignored.exe").write_bytes(b"fixture")

    def forbid_network(**kwargs: object) -> None:
        """Fail any transport creation when verified installed bytes already satisfy acquisition."""
        raise AssertionError("An installed exact model must not be downloaded again.")

    monkeypatch.setattr(aiohttp, "ClientSession", forbid_network)
    downloads = ModelArtifactDownloads(
        FixtureFileRoots(tmp_path), lambda: None, fixture_download_authorization()
    )
    assert asyncio.run(downloads.install(fixture_offer())) == "renamed.safetensors"
    assert (tmp_path / "renamed.safetensors").read_bytes() == b"fixture"
    assert (tmp_path / "wrong.safetensors").read_bytes() == b"wrong!!"
    assert not (tmp_path / "fixture.safetensors").exists()
    assert not tuple(tmp_path.glob("*.download"))


@pytest.mark.parametrize(
    "name,content",
    [
        ("wrong.safetensors", b"wrong!!"),
        ("short.safetensors", b"fix"),
        ("different-format.pth", b"fixture"),
    ],
)
def test_nonmatching_installed_artifacts_do_not_satisfy_acquisition(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, name: str, content: bytes
) -> None:
    """Preserve nearby files while requiring exact hash, size, and compatible format for reuse."""
    (tmp_path / name).write_bytes(content)
    session = TransportSession(
        TransportResponse(200, b"fixture"), expected_timeout=7200
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    downloads = ModelArtifactDownloads(
        FixtureFileRoots(tmp_path), lambda: None, fixture_download_authorization()
    )
    assert asyncio.run(downloads.install(fixture_offer())) == "fixture.safetensors"
    assert (tmp_path / name).read_bytes() == content
    assert (tmp_path / "fixture.safetensors").read_bytes() == b"fixture"
    assert len(session.requests) == 1
    assert not tuple(tmp_path.glob("*.download"))


@pytest.mark.parametrize(
    "target",
    [
        "http://github.com/fixture.pth",
        "https://127.0.0.1/fixture.pth",
        "https://localhost/fixture.pth",
        "https://secret@github.com/fixture.pth",
        "https://github.com:444/fixture.pth",
        "https://github.com/fixture.pth#fragment",
    ],
)
def test_openmodeldb_unsafe_redirect_is_rejected_and_partial_file_removed(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, target: str
) -> None:
    """Stop unsafe delivery before its second request and clean the normal temporary-file lifetime."""
    session = OpenModelDbRedirectSession(target)
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    offer = replace(
        fixture_offer(),
        file_name="fixture.pth",
        kind="upscale_models",
        provider="openmodeldb",
        download_url="https://github.com/example/fixture.pth",
    )
    with pytest.raises(ValueError, match="outside reviewed"):
        asyncio.run(
            ModelArtifactDownloads(
                FixtureFileRoots(tmp_path, "upscale_models"),
                lambda: None,
                fixture_download_authorization(),
            ).install(offer)
        )
    assert len(session.requests) == 1 and not tuple(tmp_path.iterdir())


def test_cancellation_after_atomic_publication_returns_installed_file(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Treat cancellation after the actual filesystem commit as a successful verified installation."""
    published, release = Event(), Event()
    original = ModelArtifactFile.publish

    def publish(file: ModelArtifactFile) -> str:
        """Pause the worker after publication so the cancellation race is deterministic."""
        name = original(file)
        published.set()
        wait_for_event(release, "The publication proof was not released.")
        return name

    monkeypatch.setattr(ModelArtifactFile, "publish", publish)
    session = TransportSession(
        TransportResponse(200, b"fixture"), expected_timeout=7200
    )
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Cancel only after the real tiny fixture has passed SHA256 and atomic publication."""
        downloads = ModelArtifactDownloads(
            FixtureFileRoots(tmp_path), lambda: None, fixture_download_authorization()
        )
        task = asyncio.create_task(downloads.install(fixture_offer()))
        await asyncio.to_thread(wait_for_event, published)
        task.cancel()
        release.set()
        assert await task == "fixture.safetensors"

    asyncio.run(exercise())
    assert (tmp_path / "fixture.safetensors").read_bytes() == b"fixture"
    assert not tuple(tmp_path.glob("*.download"))


class PausedDownloadBody(TransportBody):
    """Pause after a fixture chunk to observe a cancellation with real temporary-file IO."""

    def __init__(self, paused: asyncio.Event) -> None:
        """Bind a deterministic pause signal at the network stream boundary."""
        super().__init__(b"fixture")
        self._paused = paused

    async def iter_chunked(self, size: int) -> AsyncIterator[bytes]:
        """Yield a small chunk and wait until the requesting job cancels the stream."""
        yield b"fix"
        self._paused.set()
        await asyncio.Future[None]()


def test_download_cancellation_removes_temporary_file(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Cancel an active transfer and leave neither partial model nor stale temporary bytes."""

    async def exercise() -> None:
        """Observe the streaming boundary before requesting cancellation."""
        paused = asyncio.Event()
        response = TransportResponse(200, b"fixture")
        response.content = PausedDownloadBody(paused)
        session = TransportSession(response, expected_timeout=7200)
        monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
        downloads = ModelArtifactDownloads(
            FixtureFileRoots(tmp_path), lambda: None, fixture_download_authorization()
        )
        task = asyncio.create_task(downloads.install(fixture_offer()))
        await paused.wait()
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task

    asyncio.run(exercise())
    assert not tuple(tmp_path.iterdir())


class PrivateAddressResolver:
    """Replace platform DNS with a nonpublic endpoint for an explicit security-boundary proof."""

    async def resolve(
        self, host: str, port: int = 0, family: socket.AddressFamily = socket.AF_INET
    ) -> list[ResolveResult]:
        """Return the same typed address shape as the external platform resolver."""
        return [
            ResolveResult(
                hostname=host,
                host="127.0.0.1",
                port=port,
                family=family,
                proto=0,
                flags=0,
            )
        ]

    async def close(self) -> None:
        """Release the isolated resolver fixture."""


def test_download_dns_rejects_nonpublic_endpoints(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Block reviewed hostnames resolving to loopback before a connection can be opened."""
    monkeypatch.setattr(aiohttp.resolver, "DefaultResolver", PrivateAddressResolver)

    async def exercise() -> None:
        """Use the public resolver operation rather than inspecting internal guard state."""
        resolver = PublicModelDownloadResolver()
        with pytest.raises(ValueError, match="public address"):
            await resolver.resolve("civitai.com", 443)
        await resolver.close()

    asyncio.run(exercise())
