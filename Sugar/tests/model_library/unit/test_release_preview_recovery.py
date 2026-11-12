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
"""Prove version preview cache reuse, physical repair and independent cancellation."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

import asyncio
from dataclasses import replace
from pathlib import Path

import pytest

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.enrichment_service import (
    ModelPreviewUnavailableError,
)
from sugarcubes.model_library.application.release_previews import ModelReleasePreviews
from sugarcubes.model_library.application.metadata_activity import ModelMetadataActivity
from sugarcubes.model_library.application.release_service import ModelReleaseService
from sugarcubes.model_library.domain.models import ModelMetadata, ThumbnailReference
from sugarcubes.model_library.domain.provider_releases import ModelProviderRelease
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.model_usage_store import (
    SqliteModelUsageRepository,
)
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from tests.model_library.support.model_roots import FixtureFileRoots
from tests.model_library.support.model_releases import release
from tests.model_library.support.model_updates import NoDownloads
from tests.model_library.support.thumbnail_freshness import PreparedFixturePreviews
from tests.support.no_progress import await_outcome


class PreviewFamilyProvider:
    """Supply changing remote preview policy through the real family eligibility owner."""

    def __init__(self) -> None:
        """Start with a permitted image on the exact installed release."""
        self.preview: str | None = "https://image.civitai.com/fixture.jpg"

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Keep compatibility evidence stable while changing only remote image availability."""
        assert model_id == "10"
        current = release("20")
        return (
            replace(
                current, version=replace(current.version, preview_url=self.preview)
            ),
        )


class DelayedPreviews(PreparedFixturePreviews):
    """Control the external image boundary without mocking cache or family state."""

    def __init__(self, assets: ModelThumbnailAssets) -> None:
        """Expose deterministic preparation entry and release signals."""
        super().__init__(assets)
        self.entered = asyncio.Event()
        self.proceed = asyncio.Event()

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Hold an independent provider image operation until the test releases it."""
        self.entered.set()
        await self.proceed.wait()
        return await super().prepare(url)


def fixture(
    root: Path,
    previews: PreparedFixturePreviews,
) -> tuple[ModelReleasePreviews, str, PreviewFamilyProvider, ModelReleaseService]:
    """Compose actual catalog and release eligibility around tiny local file evidence."""
    (root / "installed.safetensors").write_bytes(b"fixture")
    repository = SqliteModelCatalogRepository(root)
    ModelCatalogService(
        FixtureFileRoots(root), repository, model_relationships()
    ).refresh()
    entry = repository.list_entries()[0]
    repository.save_fingerprint(entry.evidence, "b" * 64)
    repository.save_provider_metadata(
        "b" * 64,
        (
            ProviderMetadataRecord(
                ModelMetadata(
                    "Fixture",
                    base_model="SDXL 1.0",
                    provider="civitai",
                    provider_model_id="10",
                    provider_version_id="20",
                )
            ),
        ),
    )
    provider = PreviewFamilyProvider()
    service = ModelReleaseService(
        repository,
        provider,
        NoDownloads(),
        SqliteModelUsageRepository(root / "usage.sqlite3"),
    )
    return (
        ModelReleasePreviews(service, previews, ModelMetadataActivity()),
        entry.evidence.location_id,
        provider,
        service,
    )


def test_family_preview_repairs_removed_files_and_retries_failed_repair(
    tmp_path: Path,
) -> None:
    """Keep a URL cache from returning dead assets after collection or filesystem loss."""

    async def exercise() -> None:
        """Reuse available files, rebuild missing bytes and recover after a transport failure."""
        images = PreparedFixturePreviews(ModelThumbnailAssets(tmp_path / "assets"))
        previews, identity, provider, service = fixture(tmp_path, images)
        try:
            first = await previews.get(identity, "20")
            assert await previews.get(identity, "20") == first
            assert len(images.urls) == 1
            images.assets.clear_sources("civitai")
            assert not images.available(first)
            restored = await asyncio.gather(
                previews.get(identity, "20"), previews.get(identity, "20")
            )
            assert all(item == first for item in restored)
            assert images.available(first)
            assert len(images.urls) == 2
            images.assets.clear_sources("civitai")
            images.unavailable = True
            with pytest.raises(ModelPreviewUnavailableError):
                await previews.get(identity, "20")
            images.unavailable = False
            assert await previews.get(identity, "20") == first
            assert images.available(first)
            assert len(images.urls) == 4
            provider.preview = None
            await service.family(identity, refresh=True)
            assert await previews.get(identity, "20") == ()
            assert len(images.urls) == 4
        finally:
            await previews.close()

    asyncio.run(exercise())


def test_cancelling_one_family_consumer_keeps_shared_preparation(
    tmp_path: Path,
) -> None:
    """Keep one dismissed modal from cancelling another consumer's matching image."""

    async def exercise() -> None:
        """Wait on explicit signals instead of timing-dependent sleeps."""
        images = DelayedPreviews(ModelThumbnailAssets(tmp_path / "assets"))
        previews, identity, _provider, _service = fixture(tmp_path, images)
        try:
            first = asyncio.create_task(previews.get(identity, "20"))
            await await_outcome(images.entered.wait())
            second = asyncio.create_task(previews.get(identity, "20"))
            first.cancel()
            with pytest.raises(asyncio.CancelledError):
                await first
            images.proceed.set()
            assert images.available(await await_outcome(second))
            assert len(images.urls) == 1
            assert images.available(await previews.get(identity, "20"))
            assert len(images.urls) == 1
        finally:
            await previews.close()

    asyncio.run(exercise())
