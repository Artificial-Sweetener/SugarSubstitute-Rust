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
"""Prove shared update and chronology presentation preserves authoritative selection evidence."""

from __future__ import annotations

from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

import asyncio
from dataclasses import replace
from pathlib import Path

from aiohttp import web
from aiohttp.test_utils import TestClient, TestServer
import pytest

from sugarcubes.model_library import host_api
from sugarcubes.model_library.application.model_update_feed import ModelUpdateFeed
from sugarcubes.model_library.domain.models import ModelMetadata
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease
from sugarcubes.model_library.infrastructure.runtime import ModelLibraryRuntime
from sugarcubes.model_library.release_api import register_model_release_routes
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.model_updates import UpdateProvider, fixture
from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.release_service import ModelReleaseFamily
from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.infrastructure.model_release_payloads import (
    model_release_history_payload,
)


def test_versions_resolve_renamed_alternate_artifacts_to_local_host_filenames(
    tmp_path: Path,
) -> None:
    """Recognize any installed eligible file rather than only the provider's primary offer."""
    service, entry, _ = fixture(tmp_path)
    (tmp_path / "renamed.safetensors").write_bytes(b"alternate")
    ModelCatalogService(
        FixtureFileRoots(tmp_path), service.catalog, model_relationships()
    ).refresh()
    alternate = next(
        item
        for item in service.catalog.list_entries()
        if item.evidence.relative_path == "renamed.safetensors"
    )
    service.catalog.save_fingerprint(alternate.evidence, "c" * 64)
    releases = asyncio.run(PreviewUpdateProvider().releases("10"))
    payload = model_release_history_payload(
        service.catalog, ModelReleaseFamily(entry, releases), entry.evidence.location_id
    )
    versions = payload["versions"]
    assert isinstance(versions, list)
    assert versions[0]["onDisk"] is True
    assert versions[0]["installedFileName"] == "renamed.safetensors"
    assert versions[1]["installedFileName"] == "installed.safetensors"
    alternate_current = replace(alternate, sha256="c" * 64, metadata=entry.metadata)
    alternate_payload = model_release_history_payload(
        service.catalog,
        ModelReleaseFamily(alternate_current, releases),
        alternate_current.evidence.location_id,
    )
    alternate_versions = alternate_payload["versions"]
    assert isinstance(alternate_versions, list)
    assert alternate_versions[1]["installedFileName"] == "renamed.safetensors"


class PreviewUpdateProvider(UpdateProvider):
    """Supply multiple eligible files and private image URLs at the external provider boundary."""

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Keep projection proofs independent of image and model transfers."""
        return tuple(
            replace(
                release,
                version=replace(
                    release.version,
                    preview_url="https://images.civitai.com/fixture.jpeg",
                ),
                offers=(
                    *release.offers,
                    replace(
                        release.offers[0],
                        file_id="121",
                        file_name="alternate.safetensors",
                        sha256="c" * 64,
                        size_bytes=99,
                    ),
                ),
            )
            for release in await super().releases(model_id)
        )


@pytest.mark.parametrize("catalog_identity", [True, False])
def test_desktop_and_browser_share_family_and_complete_update_evidence(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, catalog_identity: bool
) -> None:
    """Expose chronology, installed state and exact offers without filesystem or transfer URLs."""
    service, entry, _ = fixture(tmp_path)
    provider = PreviewUpdateProvider()
    service.releases = replace(service.releases, provider=provider)
    service.record_usage(entry)
    if not catalog_identity:
        service.catalog.save_provider_metadata(
            entry.sha256 or "", (ProviderMetadataRecord(ModelMetadata("Local title")),)
        )
    feed = ModelUpdateFeed(service)
    feed.set_enabled(True)
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    monkeypatch.setattr(runtime, "update_feed", feed)
    monkeypatch.setattr(runtime, "releases", service.releases)
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", runtime)
    routes = web.RouteTableDef()
    register_model_release_routes(routes, runtime)
    app = web.Application()
    app.add_routes(routes)

    async def exercise() -> None:
        """Compare public projections while preserving the installed fixture byte for byte."""
        try:
            snapshot = await host_api.check_model_updates()
            proposals = snapshot["proposals"]
            assert isinstance(proposals, list) and len(proposals) == 1
            proposal = proposals[0]
            assert isinstance(proposal, dict)
            current = proposal["current"]
            assert isinstance(current, dict)
            assert current["providerModelId"] == "10"
            assert current["versionId"] == "20"
            assert current["kind"] == "loras"
            assert current["baseModel"] == "SDXL"
            assert current["usageCount"] == 1
            assert current["lastUsedAt"] == "2026-10-03T00:00:00+00:00"
            candidate = proposal["candidate"]
            assert isinstance(candidate, dict)
            assert candidate["versionId"] == "21"
            assert candidate["providerModelId"] == "10"
            assert candidate["files"] == [
                {
                    "access": "unknown",
                    "fileId": "21",
                    "fileName": "21.safetensors",
                    "sha256": "a" * 64,
                    "sizeBytes": 10,
                    "kind": "loras",
                },
                {
                    "access": "unknown",
                    "fileId": "121",
                    "fileName": "alternate.safetensors",
                    "sha256": "c" * 64,
                    "sizeBytes": 99,
                    "kind": "loras",
                },
            ]
            assert candidate["previewUrl"] == (
                f"/sugarcubes/v1/models/{entry.evidence.location_id}/versions/21/preview"
            )
            history = await host_api.model_release_history(entry.evidence.location_id)
            async with TestClient(TestServer(app)) as client:
                response = await client.get(
                    f"/sugarcubes/v1/models/{entry.evidence.location_id}/versions"
                )
                assert response.status == 200
                assert await response.json() == history
            versions = history["versions"]
            assert isinstance(versions, list)
            assert [item["versionId"] for item in versions] == ["20", "21"]
            assert [item["onDisk"] for item in versions] == [True, False]
            assert [item["inUse"] for item in versions] == [True, False]
            assert versions[1]["files"] == candidate["files"]
            assert "downloadUrl" not in str(snapshot) + str(history)
            assert "api/download" not in str(snapshot) + str(history)
            assert "images.civitai.com" not in str(snapshot) + str(history)
            assert str(tmp_path) not in str(snapshot) + str(history)
            versions.clear()
            fresh_history = await host_api.model_release_history(
                entry.evidence.location_id
            )
            fresh_versions = fresh_history["versions"]
            assert isinstance(fresh_versions, list) and len(fresh_versions) == 2
            assert provider.calls == 4
            assert (tmp_path / "installed.safetensors").read_bytes() == b"fixture"
        finally:
            await feed.close()
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())


def test_family_host_rejects_absent_authority(monkeypatch: pytest.MonkeyPatch) -> None:
    """Keep sibling calls from creating fallback provider, storage or selection owners."""
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", None)
    with pytest.raises(RuntimeError, match="not active"):
        asyncio.run(host_api.model_release_history("a" * 64))


def test_shared_family_rejects_unknown_identity_and_unidentified_base(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep desktop exploration behind the same installed evidence and compatibility boundary."""
    service, entry, provider = fixture(tmp_path)
    runtime = ModelLibraryRuntime(
        FixtureFileRoots(tmp_path), enrichment_provider=ObservedProvider()
    )
    monkeypatch.setattr(runtime, "releases", service.releases)
    monkeypatch.setattr(host_api, "_ACTIVE_LIBRARY", runtime)

    async def exercise() -> None:
        """Fail closed without translating an arbitrary input into a provider request."""
        try:
            for unknown in (
                "",
                "../private",
                "https://civitai.com/models/10",
                "a" * 64,
            ):
                with pytest.raises(ValueError, match="not found"):
                    await host_api.model_release_history(unknown)
            assert provider.calls == 0
            assert entry.sha256 is not None
            runtime.repository.save_provider_metadata(
                entry.sha256,
                (
                    ProviderMetadataRecord(
                        ModelMetadata(
                            "Fixture",
                            base_model="",
                            provider="civitai",
                            provider_model_id="10",
                            provider_version_id="20",
                        )
                    ),
                ),
            )
            assert await host_api.model_release_history(entry.evidence.location_id) == {
                "schemaVersion": 1,
                "versions": [],
            }
            assert (tmp_path / "installed.safetensors").read_bytes() == b"fixture"
        finally:
            await runtime.update_feed.close()
            await runtime.automatic_enrichment.close()
            await runtime.jobs.close()

    asyncio.run(exercise())
