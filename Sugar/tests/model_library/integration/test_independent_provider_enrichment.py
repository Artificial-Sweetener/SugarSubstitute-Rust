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
"""Prove exact provider batches remain independent through enrichment, restart and maintenance."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
import asyncio
from dataclasses import replace
from pathlib import Path

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.enrichment_service import (
    ModelEnrichmentService,
)
from sugarcubes.model_library.application.fingerprint_service import (
    ModelFingerprintService,
)
from sugarcubes.model_library.application.provider_registry import (
    ModelMetadataProviders,
    RegisteredModelProvider,
)
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.models import ThumbnailReference
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.fingerprints import (
    ApprovedModelFingerprints,
)
from tests.model_library.support.enrichment import ObservedPreviews
from tests.model_library.support.model_roots import FixtureFileRoots


class ExactHashProvider:
    """Supply validated provider evidence at the actual external lookup boundary."""

    def __init__(self, version: ProviderModelVersion) -> None:
        """Retain the complete match with independent provider identity and labels."""
        self.version = version

    async def lookup(self, sha256: str) -> ProviderModelVersion:
        """Return deterministic evidence without contacting external providers."""
        return self.version


class ObservedImagePreviews(ObservedPreviews):
    """Observe which independent provider supplies the selected image."""

    def __init__(self) -> None:
        """Keep image requests visible without external transport or physical cache files."""
        self.urls: list[str] = []

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Record the selected image source through the production preparation seam."""
        self.urls.append(url)
        return await super().prepare(url)


def test_civitai_image_fallback_preserves_openmodeldb_labels_and_independent_records(
    tmp_path: Path,
) -> None:
    """Keep ordered exact links and secondary labels when the primary provider has no image."""
    (tmp_path / "selected.pth").write_bytes(b"tiny upscaler identity fixture")
    roots = FixtureFileRoots(tmp_path, "upscale_models")
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    first = ProviderModelVersion(
        "openmodeldb",
        "4x-Fixture",
        "4x-Fixture",
        "OpenDB title",
        "v3",
        "SPAN",
        "https://openmodeldb.info/models/4x-Fixture",
        "OpenDB description",
        ("local",),
        None,
    )
    second = replace(
        first,
        provider="civitai",
        model_id="10",
        version_id="20",
        title="CivitAI title",
        version="v2",
        page_url="https://civitai.com/models/10",
        preview_url="https://image.civitai.com/fixture.jpg",
    )
    providers = ModelMetadataProviders(
        tuple(
            RegisteredModelProvider(
                frozenset({"upscale_models"}), ExactHashProvider(item)
            )
            for item in (first, second)
        )
    )
    previews = ObservedImagePreviews()
    service = ModelEnrichmentService(
        repository,
        ModelFingerprintService(repository, ApprovedModelFingerprints(roots)),
        providers,
        previews,
    )
    result = asyncio.run(service.refresh(repository.list_entries()[0]))
    current = repository.list_entries()[0]
    assert (
        result.metadata == current.metadata
        and current.metadata is not None
        and current.sha256 is not None
    )
    assert (
        current.metadata.title == first.title
        and current.metadata.version == first.version
    )
    assert current.thumbnail_origin == "civitai" and current.metadata.thumbnails
    assert previews.urls == [second.preview_url]
    assert repository.provider_cache.metadata(
        current.sha256, "openmodeldb"
    ) == first.metadata(())
    assert repository.provider_cache.metadata(
        current.sha256, "civitai"
    ) == second.metadata(current.metadata.thumbnails)
    reopened = SqliteModelCatalogRepository(tmp_path)
    assert reopened.list_entries() == (current,)
    reopened.provider_cache.clear_metadata("openmodeldb")
    remaining = reopened.list_entries()[0]
    assert remaining.evidence == current.evidence and remaining.sha256 == current.sha256
    assert remaining.metadata is not None and remaining.metadata.title == second.title
    assert (
        remaining.metadata.thumbnails == current.metadata.thumbnails
        and remaining.thumbnail_origin == "civitai"
    )
    reopened.provider_cache.clear_thumbnails("civitai")
    final = reopened.list_entries()[0]
    assert final.metadata is not None and final.metadata.thumbnails == ()
    assert (
        final.metadata.title == second.title
        and final.metadata.links() == second.metadata(()).links()
    )
