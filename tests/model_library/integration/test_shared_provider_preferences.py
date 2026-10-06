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
"""Prove shared production provider policy across HTTP, host APIs and live adapter settings."""

from __future__ import annotations

import asyncio
import json
from pathlib import Path

import aiohttp
from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
import pytest

from sugarcubes.model_library import host_api
from sugarcubes.model_library.domain.thumbnail_safety import CivitaiThumbnailSafety
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from sugarcubes.model_library.domain.provider_preferences import (
    ModelProviderPreferences,
    ModelProviderPreferencePatch,
)
from sugarcubes.model_library.preference_api import register_model_preference_routes
from tests.model_library.support.enrichment import ObservedPreviews
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)
from sugarcubes.model_library.application.requested_artifacts import (
    RequestedModelArtifacts,
)


def test_host_and_http_share_persistent_policy_without_provider_traffic(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Merge HTTP and host edits without touching update consent, model files or workflow values."""
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", runtime)
    routes = web.RouteTableDef()
    register_model_preference_routes(routes, runtime.preferences)
    app = web.Application()
    app.add_routes(routes)

    async def exercise() -> None:
        """Observe one preference owner through real local HTTP and direct host contracts."""
        try:
            async with TestClient(TestServer(app)) as client:
                initial = host_api.prepared_model_provider_preferences()
                initial_updates = host_api.prepared_model_updates()
                assert initial_updates["enabled"] is True
                assert initial == {
                    "schemaVersion": 1,
                    "metadataLookupEnabled": True,
                    "missingModelLookupEnabled": True,
                    "downloadsEnabled": True,
                    "thumbnailDownloadsEnabled": True,
                    "thumbnailSafety": "sfw_only",
                }
                assert (
                    await (await client.get("/sugarcubes/v1/models/preferences")).json()
                    == initial
                )
                saved = host_api.configure_model_provider_preferences(
                    {"thumbnailSafety": "allow_soft"}
                )
                assert (
                    await (await client.get("/sugarcubes/v1/models/preferences")).json()
                    == saved
                )
                response = await client.patch(
                    "/sugarcubes/v1/models/preferences",
                    json={"metadataLookupEnabled": False},
                )
                assert response.status == 200
                assert (await response.json())["thumbnailSafety"] == "allow_soft"
                assert not runtime.preferences.metadata_lookup_enabled()
                for payload in (
                    {},
                    {"metadataLookupEnabled": "false"},
                    {"thumbnailSafety": "invalid"},
                    {"apiKey": "bad"},
                ):
                    assert (
                        await client.patch(
                            "/sugarcubes/v1/models/preferences", json=payload
                        )
                    ).status == 400
                assert (
                    await client.patch(
                        "/sugarcubes/v1/models/preferences",
                        data="{",
                        headers={"Content-Type": "application/json"},
                    )
                ).status == 400
                assert (
                    host_api.prepared_model_provider_preferences()[
                        "metadataLookupEnabled"
                    ]
                    is False
                )
                assert host_api.prepared_model_updates() == initial_updates
                response = await client.patch(
                    "/sugarcubes/v1/models/preferences",
                    json={"thumbnailDownloadsEnabled": False},
                )
                assert response.status == 200
                assert (
                    runtime.preferences.image_policy()
                    is CivitaiThumbnailSafety.DISABLED
                )
                restarted = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
                assert restarted.preferences.read() == runtime.preferences.read()
                await restarted.jobs.close()
        finally:
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_explicit_missing_model_lookup_is_independent_of_metadata_opt_out(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Resolve an explicit recipe offer without enabling automatic installed-model enrichment."""
    version: dict[str, object] = {
        "id": 20,
        "modelId": 10,
        "name": "v1",
        "baseModel": "SDXL 1.0",
        "model": {"name": "Fixture", "type": "LORA"},
        "files": [
            {
                "id": 30,
                "type": "Model",
                "name": "fixture.safetensors",
                "metadata": {"format": "SafeTensor"},
                "sizeKB": 7 / 1024,
                "hashes": {"SHA256": "a" * 64},
                "downloadUrl": "https://civitai.com/api/download/models/20",
                "pickleScanResult": "Success",
                "virusScanResult": "Success",
            }
        ],
    }

    class RecipeSession(TransportSession):
        """Return hash identity or current history only at the external HTTP seam."""

        def get(
            self,
            url: str,
            *,
            allow_redirects: bool,
            headers: dict[str, str] | None = None,
        ) -> TransportResponse:
            """Keep lookup and offer verification inside production provider adapters."""
            payload = (
                {
                    "id": 10,
                    "name": "Fixture",
                    "type": "LORA",
                    "modelVersions": [version],
                }
                if url.endswith("/models/10")
                else version
            )
            if "/model-versions/mini/" in url:
                payload = {
                    "availability": "Public",
                    "requireAuth": False,
                    "checkPermission": False,
                }
            self.response = TransportResponse(200, json.dumps(payload).encode())
            return super().get(url, allow_redirects=allow_redirects, headers=headers)

    session = RecipeSession(TransportResponse(200, b"{}"))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))
    runtime.preferences.configure({"metadata_lookup_enabled": False})
    provider = runtime.releases.provider
    assert isinstance(provider, CivitaiModelProvider)

    async def exercise() -> None:
        """Observe real hash lookup, history parsing, and live preference changes at HTTP seam."""
        try:
            assert await provider.lookup("a" * 64) is None
            assert not session.requests
            offer = await runtime.desktop_acquisitions.artifacts.offer(
                "a" * 64, "loras"
            )
            assert offer.sha256 == "a" * 64 and offer.file_id == "30"
            assert len(session.requests) == 3
            count = len(session.requests)
            runtime.preferences.configure({"missing_model_lookup_enabled": False})
            with pytest.raises(ValueError, match="verified"):
                await RequestedModelArtifacts(provider).offer("a" * 64, "loras")
            assert len(session.requests) == count
            runtime.preferences.configure({"missing_model_lookup_enabled": True})
            assert (
                await RequestedModelArtifacts(provider).offer("a" * 64, "loras")
            ) == offer
            assert len(session.requests) == count + 3
            assert not runtime.preferences.metadata_lookup_enabled()
        finally:
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_runtime_rechecks_policy_for_metadata_and_family_without_negative_caching_disabled_work(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep opt-out free of hash/network work and select current policy after enabling the same runtime."""
    model = tmp_path / "fixture.safetensors"
    original = b"tiny policy fixture"
    model.write_bytes(original)
    version: dict[str, object] = {
        "id": 20,
        "modelId": 10,
        "name": "v1",
        "baseModel": "SDXL 1.0",
        "model": {"name": "Fixture", "type": "LORA"},
        "images": [{"url": "https://image.civitai.com/soft.jpg", "nsfwLevel": 2}],
    }

    class PolicySession(TransportSession):
        """Return complete release history for the initial metadata update check."""

        def get(
            self,
            url: str,
            *,
            allow_redirects: bool,
            headers: dict[str, str] | None = None,
        ) -> TransportResponse:
            """Keep metadata and history requests distinct at the external HTTP seam."""
            response = super().get(
                url, allow_redirects=allow_redirects, headers=headers
            )
            if url == "https://civitai.com/api/v1/models/10":
                return TransportResponse(
                    200,
                    json.dumps(
                        {
                            "id": 10,
                            "name": "Fixture",
                            "type": "LORA",
                            "modelVersions": [version],
                        }
                    ).encode(),
                )
            return response

    session = PolicySession(TransportResponse(200, json.dumps(version).encode()))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_previews=ObservedPreviews()
    )
    runtime.catalog.refresh()
    entry = runtime.repository.list_entries()[0]
    runtime.preferences.configure({"metadata_lookup_enabled": False})

    async def exercise() -> None:
        """Use composed metadata jobs and family provider at the external transport seam."""
        try:
            disabled = await runtime.refresh_metadata(entry.evidence.location_id)
            await runtime.jobs.wait(disabled.job_id)
            assert disabled.result["status"] == "disabled"
            assert session.requests == []
            assert runtime.repository.list_entries()[0].sha256 is None
            runtime.preferences.configure({"metadata_lookup_enabled": True})
            enabled = await runtime.refresh_metadata(entry.evidence.location_id)
            await runtime.jobs.wait(enabled.job_id)
            assert enabled.result["status"] == "found"
            current = runtime.repository.list_entries()[0]
            assert current.metadata is not None and not current.metadata.thumbnails
            assert len(session.requests) == 2
            runtime.preferences.configure(
                {"thumbnail_safety": CivitaiThumbnailSafety.ALLOW_SOFT}
            )
            refreshed = await runtime.refresh_metadata(entry.evidence.location_id)
            await runtime.jobs.wait(refreshed.job_id)
            assert refreshed.result["status"] == "found"
            current = runtime.repository.list_entries()[0]
            assert current.metadata is not None and current.metadata.thumbnails
            session.response = TransportResponse(
                200,
                json.dumps(
                    {
                        "id": 10,
                        "name": "Fixture",
                        "type": "LORA",
                        "modelVersions": [version],
                    }
                ).encode(),
            )
            assert (await runtime.releases.provider.releases("10"))[
                0
            ].version.preview_url == "https://image.civitai.com/soft.jpg"
            runtime.preferences.configure({"thumbnail_downloads_enabled": False})
            assert (await runtime.releases.provider.releases("10"))[
                0
            ].version.preview_url is None
            provider = runtime.releases.provider
            assert isinstance(provider, CivitaiModelProvider)
            requests = len(session.requests)
            with pytest.raises(ValueError, match="disabled"):
                await provider.read_preview("https://image.civitai.com/stale.jpg")
            assert len(session.requests) == requests
            assert model.read_bytes() == original
            assert (
                runtime.repository.list_entries()[0].evidence.value
                == entry.evidence.value
            )
        finally:
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_unavailable_host_cannot_create_a_second_preference_owner(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Require the running authority for shared preferences instead of inventing local defaults."""
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", None)
    with pytest.raises(RuntimeError, match="not active"):
        host_api.prepared_model_provider_preferences()
    with pytest.raises(RuntimeError, match="not active"):
        host_api.configure_model_provider_preferences(
            {"thumbnailDownloadsEnabled": False}
        )


@pytest.mark.parametrize("method", ["GET", "PATCH"])
def test_preference_storage_failures_return_owned_http_feedback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, method: str
) -> None:
    """Keep storage diagnostics server-side while returning actionable settings feedback."""
    runtime = ModelLibraryRuntime(FixtureFileRoots(tmp_path))

    def failed_read() -> ModelProviderPreferences:
        """Model a failed storage boundary without altering real persisted defaults."""
        raise OSError("Storage unavailable.")

    def failed_save(_patch: ModelProviderPreferencePatch) -> ModelProviderPreferences:
        """Model a failed transaction at the same application boundary."""
        raise OSError("Storage unavailable.")

    monkeypatch.setattr(runtime.preferences, "read", failed_read)
    monkeypatch.setattr(runtime.preferences, "configure", failed_save)
    routes = web.RouteTableDef()
    register_model_preference_routes(routes, runtime.preferences)
    app = web.Application()
    app.add_routes(routes)

    async def exercise() -> None:
        """Use HTTP errors without opening browser-native or blocking product dialogs."""
        try:
            async with TestClient(TestServer(app)) as client:
                response = await client.request(
                    method,
                    "/sugarcubes/v1/models/preferences",
                    json={"metadataLookupEnabled": False},
                )
                assert response.status == 500
                assert "Storage unavailable" not in await response.text()
        finally:
            await runtime.jobs.close()

    asyncio.run(exercise())
