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
"""Prove provider HTTP bounds, authentication isolation, cooldown, and missing-result caching."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
import asyncio
import json
from pathlib import Path
import time
import ssl

import aiohttp
import pytest

from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)

from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from sugarcubes.model_library.infrastructure.civitai_transport import CivitaiTransport
from sugarcubes.model_library.infrastructure.civitai_family_discovery import (
    CivitaiFamilyDiscovery,
)
from sugarcubes.model_library.domain.thumbnail_safety import CivitaiThumbnailSafety
from sugarcubes.model_library.domain.model_suggestions import (
    ModelSuggestionContext,
    ModelSuggestionAccessPolicy,
)
from sugarcubes.model_library.infrastructure.provider_tls import provider_tls_context
from sugarcubes.model_library.infrastructure.openmodeldb_client import (
    OpenModelDbProvider,
)
from sugarcubes.model_library.infrastructure.provider_catalog_cache import (
    ProviderCatalogCache,
    ProviderCatalogSnapshot,
)


def test_provider_tls_keeps_certificate_and_hostname_verification() -> None:
    """Use explicit trusted roots without weakening provider identity verification."""
    context = provider_tls_context()
    assert context.verify_mode == ssl.CERT_REQUIRED
    assert context.check_hostname
    assert context.get_ca_certs()
    assert provider_tls_context() is context


def test_metadata_and_history_share_the_provider_cooldown(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Keep history from bypassing a rate limit encountered during metadata lookup."""
    session = TransportSession(TransportResponse(429, b"{}"))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Observe the same public provider across both request operations."""
        provider = CivitaiModelProvider()
        with pytest.raises(ValueError, match="HTTP 429"):
            await provider.lookup("a" * 64)
        with pytest.raises(ValueError, match="cooldown"):
            await provider.releases("1")

    asyncio.run(exercise())
    assert len(session.requests) == 1


def test_discovery_cannot_bypass_metadata_rate_limits(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Share provider cooldown across independently owned metadata and family-discovery adapters."""
    session = TransportSession(TransportResponse(429, b"{}"))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Use one transport exactly as the production runtime composes its collaborators."""
        transport = CivitaiTransport()
        with pytest.raises(ValueError, match="HTTP 429"):
            await CivitaiModelProvider(transport=transport).lookup("a" * 64)
        with pytest.raises(ValueError, match="cooldown"):
            await CivitaiFamilyDiscovery(transport, model_relationships()).discover(
                ModelSuggestionContext("checkpoints", "sdxl"),
                access_policy=ModelSuggestionAccessPolicy.CURRENT_USER,
                limit=8,
            )

    asyncio.run(exercise())
    assert len(session.requests) == 1


def test_metadata_and_history_follow_the_current_thumbnail_policy(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Avoid freezing image-content preferences into a long-lived provider instance."""
    version: dict[str, object] = {
        "id": 11,
        "modelId": 1,
        "name": "v1",
        "model": {"name": "Fixture"},
        "images": [{"url": "https://image.civitai.com/soft.jpg", "nsfwLevel": 2}],
    }
    session = TransportSession(TransportResponse(200, json.dumps(version).encode()))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    policies = [CivitaiThumbnailSafety.SFW_ONLY]

    async def exercise() -> None:
        """Change user policy without replacing either the metadata or transport owner."""
        provider = CivitaiModelProvider(thumbnail_policy=lambda: policies[0])
        default = await provider.lookup("a" * 64)
        assert default is not None and default.preview_url is None
        policies[0] = CivitaiThumbnailSafety.ALLOW_SOFT
        allowed = await provider.lookup("a" * 64)
        assert (
            allowed is not None
            and allowed.preview_url == "https://image.civitai.com/soft.jpg"
        )
        session.response = TransportResponse(
            200,
            json.dumps(
                {"id": 1, "name": "Fixture", "modelVersions": [version]}
            ).encode(),
        )
        assert (await provider.releases("1"))[
            0
        ].version.preview_url == allowed.preview_url
        policies[0] = CivitaiThumbnailSafety.DISABLED
        assert (await provider.releases("1"))[0].version.preview_url is None

    asyncio.run(exercise())


class PreviewRedirectSession(TransportSession):
    """Model CivitAI's original-image redirect without contacting either remote host."""

    def __init__(self, target: str, *, loop: bool = False) -> None:
        """Bind a reviewed or malicious redirect destination."""
        super().__init__(TransportResponse(301, b""))
        self.target = target
        self.loop = loop

    def get(
        self, url: str, *, allow_redirects: bool, headers: dict[str, str] | None = None
    ) -> TransportResponse:
        """Return a redirect followed by bounded image bytes."""
        self.response = (
            TransportResponse(301, b"")
            if not self.requests or self.loop
            else TransportResponse(200, b"prepared source image", "image/png")
        )
        self.response.headers["Location"] = self.target
        return super().get(url, allow_redirects=allow_redirects, headers=headers)


@pytest.mark.parametrize(
    "target",
    [
        "https://blobs-b2.civitai.com/file/blobs-managed-public/ABC123",
        "https://127.0.0.1/secret",
        "https://blobs-b2.civitai.com/file/private/ABC123",
        "https://image.civitai.com@127.0.0.1/secret",
    ],
)
def test_preview_redirects_admit_only_public_provider_images(
    monkeypatch: pytest.MonkeyPatch, target: str
) -> None:
    """Support the provider's real CDN without granting arbitrary redirects or leaking credentials."""
    session = PreviewRedirectSession(target)
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    provider = CivitaiModelProvider(lambda: "fixture-token")
    request = provider.read_preview("https://image.civitai.com/fixture.jpeg")
    if target == "https://blobs-b2.civitai.com/file/blobs-managed-public/ABC123":
        assert asyncio.run(request) == b"prepared source image"
        assert len(session.requests) == 2
    else:
        with pytest.raises(ValueError, match="reviewed origins"):
            asyncio.run(request)
        assert len(session.requests) == 1
    assert all("Authorization" not in headers for headers in session.headers)
    assert all(not redirects for _, redirects in session.requests)


def test_preview_redirect_loops_stop_at_the_bound(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Bound redirect traffic even when all destinations remain approved."""
    session = PreviewRedirectSession("https://image.civitai.com/loop.jpeg", loop=True)
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    with pytest.raises(ValueError, match="redirect limit"):
        asyncio.run(
            CivitaiModelProvider().read_preview(
                "https://image.civitai.com/fixture.jpeg"
            )
        )
    assert len(session.requests) == 4


def test_credentials_stay_off_the_image_cdn_and_redirects_are_disabled(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Authorize only the metadata API while keeping image requests credential-free."""
    body = json.dumps(
        {"id": 2, "modelId": 1, "name": "v1", "model": {"name": "Fixture"}}
    ).encode()
    session = TransportSession(TransportResponse(200, body))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Use the same provider instance for metadata and preview transport."""
        provider = CivitaiModelProvider(lambda: "fixture-token")
        assert await provider.lookup("a" * 64) is not None
        session.response = TransportResponse(200, b"image bytes", "image/webp")
        assert (
            await provider.read_preview("https://image.civitai.com/fixture.webp")
            == b"image bytes"
        )

    asyncio.run(exercise())
    assert session.headers[0]["Authorization"] == "Bearer fixture-token"
    assert "Authorization" not in session.headers[1]
    assert all(not redirects for _, redirects in session.requests)


@pytest.mark.parametrize("status", [404, 429, 500])
def test_missing_results_and_cooldowns_do_not_repeat_requests(
    monkeypatch: pytest.MonkeyPatch, status: int
) -> None:
    """Avoid retry storms while surfacing provider errors rather than silently succeeding."""
    session = TransportSession(TransportResponse(status, b"{}"))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Run successive public lookups with deterministic remote responses."""
        provider = CivitaiModelProvider()
        if status == 404:
            assert await provider.lookup("a" * 64) is None
            assert await provider.lookup("a" * 64) is None
        else:
            with pytest.raises(ValueError, match="unavailable"):
                await provider.lookup("a" * 64)
            with pytest.raises(ValueError, match="cooldown"):
                await provider.lookup("b" * 64)

    asyncio.run(exercise())
    assert len(session.requests) == 1


def test_openmodeldb_cache_revalidation_and_offline_fallback(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Reuse validated exact hashes across restart, HTTP 304, and provider outages."""
    payload = {
        "4x-Fixture": {
            "name": "Fixture",
            "resources": [
                {
                    "type": "pth",
                    "sha256": "a" * 64,
                    "size": 512,
                    "urls": ["https://github.com/example/fixture.pth"],
                }
            ],
            "thumbnail": {"SR": "/thumbs/fixture.jpg"},
        }
    }
    cache = ProviderCatalogCache(tmp_path / "openmodeldb.json")
    modified = "Wed, 01 Oct 2025 12:00:00 GMT"
    cache.save(ProviderCatalogSnapshot(payload, 0, '"fixture-etag"', modified))
    session = TransportSession(TransportResponse(304, b""))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Exercise the public lookup contract while controlling only transport responses."""
        provider = OpenModelDbProvider(cache)
        result = await provider.lookup("a" * 64)
        assert (
            result is not None
            and result.preview_url == "https://openmodeldb.info/thumbs/fixture.jpg"
        )
        assert await provider.lookup("b" * 64) is None
        assert len(session.requests) == 1
        assert any(
            headers.get("If-None-Match") == '"fixture-etag"'
            for headers in session.headers
        )
        assert any(
            headers.get("If-Modified-Since") == modified for headers in session.headers
        )
        restored = cache.read()
        assert restored is not None and restored.last_modified == modified
        cache.save(ProviderCatalogSnapshot(payload, 0, None))
        session.response = TransportResponse(503, b"{}")
        fallback = OpenModelDbProvider(cache)
        assert await fallback.lookup("a" * 64) is not None
        assert await fallback.lookup("a" * 64) is not None
        assert len(session.requests) == 3
        preview_session = TransportSession(
            TransportResponse(200, b"image bytes", "image/webp"), expected_timeout=10
        )
        monkeypatch.setattr(aiohttp, "ClientSession", preview_session.factory)
        assert (
            await fallback.read_preview("https://openmodeldb.info/thumbs/fixture.jpg")
            == b"image bytes"
        )
        with pytest.raises(ValueError, match="not approved"):
            await fallback.read_preview("https://127.0.0.1/secret")

    asyncio.run(exercise())
    assert all(not redirects for _, redirects in session.requests)


def test_provider_cache_preserves_future_schema_and_rejects_header_injection(
    tmp_path: Path,
) -> None:
    """Keep incompatible persisted state untouched and prevent cached validators becoming unsafe headers."""
    path = tmp_path / "provider.json"
    cache = ProviderCatalogCache(path)
    with pytest.raises(ValueError, match="ETag"):
        cache.save(ProviderCatalogSnapshot({}, time.time(), "ok\r\nsecret: value"))
    with pytest.raises(ValueError, match="Last-Modified"):
        cache.save(
            ProviderCatalogSnapshot({}, time.time(), None, "ok\r\nsecret: value")
        )
    original = b'{"schemaVersion":2,"payload":{"future":true}}'
    path.write_bytes(original)
    with pytest.raises(RuntimeError, match="preserved"):
        cache.save(ProviderCatalogSnapshot({}, time.time(), None))
    assert path.read_bytes() == original


def test_metadata_payload_size_and_unapproved_preview_addresses_fail_closed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Enforce decompressed byte bounds and reject caller-controlled image hosts before IO."""
    session = TransportSession(TransportResponse(200, b"x" * (4 * 1024 * 1024 + 1)))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)

    async def exercise() -> None:
        """Reject oversized metadata and unsafe preview requests through public operations."""
        provider = CivitaiModelProvider()
        with pytest.raises(ValueError, match="size limit"):
            await provider.lookup("a" * 64)
        with pytest.raises(ValueError, match="not approved"):
            await provider.read_preview("http://127.0.0.1/secret")

    asyncio.run(exercise())
    assert len(session.requests) == 1
