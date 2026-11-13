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
"""Compose real prepared assets with deterministic provider and clock boundaries."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from io import BytesIO
from pathlib import Path

from PIL import Image

from sugarcubes.model_library.application.automatic_enrichment import (
    AutomaticModelEnrichment,
)
from sugarcubes.model_library.application.enrichment_freshness import (
    ModelEnrichmentFreshness,
)
from sugarcubes.model_library.infrastructure.enrichment_cache import (
    SqliteModelEnrichmentCache,
)
from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.enrichment_service import (
    ModelEnrichmentService,
    ModelPreviewUnavailableError,
)
from sugarcubes.model_library.application.fingerprint_service import (
    ModelFingerprintService,
)
from sugarcubes.model_library.domain.models import ThumbnailReference
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.enrichment_attempts import (
    SqliteEnrichmentAttempts,
)
from sugarcubes.model_library.infrastructure.fingerprints import (
    ApprovedModelFingerprints,
)
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from sugarcubes.model_library.infrastructure.civitai_codec import parse_civitai_version
from tests.model_library.support.enrichment import ObservedProvider
from tests.model_library.support.model_roots import FixtureFileRoots


class PreparedFixturePreviews:
    """Publish real WebP variants without remote requests or model weights."""

    def __init__(self, assets: ModelThumbnailAssets) -> None:
        """Bind a temporary asset namespace and observable preparation calls."""
        self.assets = assets
        self.urls: list[str] = []
        self.unavailable = False

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Prepare a small image or an intentional recoverable transport failure."""
        self.urls.append(url)
        if self.unavailable:
            raise ModelPreviewUnavailableError(
                "Fixture preview is unavailable.",
                reason="fixture offline",
                expected=True,
            )
        stream = BytesIO()
        Image.new("RGB", (96, 144), "navy").save(stream, "PNG")
        return ModelThumbnailPreparer(self.assets, origin="civitai").prepare(
            stream.getvalue()
        )

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Observe actual files instead of trusting stored thumbnail references."""
        return all(
            self.assets.resolve(item.asset_id) is not None for item in thumbnails
        )


class EnrichmentFreshnessFixture:
    """Exercise production enrichment, persistence and prepared files with tiny fixtures."""

    def __init__(self, root: Path, version: ProviderModelVersion | None) -> None:
        """Create exact file evidence and an injectable clock without starting background jobs."""
        self.model = root / "fixture.safetensors"
        self.model.write_bytes(b"tiny fixture, no weights")
        self.roots = FixtureFileRoots(root)
        self.repository = SqliteModelCatalogRepository(root)
        self.catalog = ModelCatalogService(
            self.roots, self.repository, model_relationships()
        )
        self.catalog.refresh()
        self.provider = ObservedProvider(version)
        self.previews = PreparedFixturePreviews(
            ModelThumbnailAssets(self.repository.asset_root)
        )
        self.clock = [1000.0]
        self.policy = ["civitai-thumbnail-v1:sfw_only"]
        self.provider_scope = ["fixture-provider"]
        self.service = ModelEnrichmentService(
            self.repository,
            ModelFingerprintService(
                self.repository, ApprovedModelFingerprints(self.roots)
            ),
            self.provider,
            self.previews,
        )
        self.automatic = self.restart()
        self.identity = self.repository.list_entries()[0].evidence.location_id

    def restart(self) -> AutomaticModelEnrichment:
        """Reopen durable decisions while retaining the same observable external boundaries."""
        return AutomaticModelEnrichment(
            self.repository,
            self.service,
            SqliteEnrichmentAttempts(
                self.repository.asset_root.parent / "enrichment.sqlite3"
            ),
            ModelEnrichmentFreshness(
                SqliteModelEnrichmentCache(
                    self.repository.asset_root.parent / "enrichment-cache.sqlite3"
                ),
                self.previews,
                lambda _kind: self.policy[0],
                lambda _kind: self.provider_scope[0],
                lambda _kind: tuple(self.provider_scope[0].split(",")),
            ),
            lambda: self.clock[0],
        )


def fixture_provider_version(*, preview: bool = True) -> ProviderModelVersion:
    """Return validated provider evidence with an optional eligible image."""
    return parse_civitai_version(
        {
            "id": 20,
            "modelId": 10,
            "name": "v2",
            "model": {"name": "Fixture"},
            "images": (
                [{"url": "https://image.civitai.com/fixture.jpg", "nsfwLevel": 1}]
                if preview
                else []
            ),
        }
    )
