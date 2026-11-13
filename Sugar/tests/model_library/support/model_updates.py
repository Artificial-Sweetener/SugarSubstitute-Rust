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
"""Provide isolated installed evidence and deterministic provider releases for update proofs."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path


from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.application.model_update_service import ModelUpdateService
from sugarcubes.model_library.application.release_service import ModelReleaseService
from sugarcubes.model_library.domain.models import ModelMetadata, ModelCatalogEntry
from sugarcubes.model_library.domain.provider_models import ProviderModelVersion
from sugarcubes.model_library.domain.provider_releases import (
    ModelArtifactOffer,
    ModelProviderRelease,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.model_usage_store import (
    SqliteModelUsageRepository,
)
from tests.model_library.support.model_roots import FixtureFileRoots

NOW = datetime(2026, 10, 3, tzinfo=UTC)


class UpdateProvider:
    """Observe provider calls and supply ordered compatible evidence without network activity."""

    def __init__(self) -> None:
        """Start with one newer version of the installed family."""
        self.calls = 0
        self.latest = "21"

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Return safe same-base releases in CivitAI's newest-first order."""
        self.calls += 1
        return tuple(
            ModelProviderRelease(
                ProviderModelVersion(
                    "civitai",
                    model_id,
                    version,
                    "Fixture",
                    version,
                    "SDXL",
                    "https://civitai.com/models/10",
                    "",
                    (),
                    None,
                    model_type="LORA",
                ),
                "",
                (
                    ModelArtifactOffer(
                        version,
                        f"{version}.safetensors",
                        ("b" if version == "20" else "a") * 64,
                        10,
                        f"https://civitai.com/api/download/models/{version}",
                        "loras",
                    ),
                ),
            )
            for version in (self.latest, "20")
        )


class NoDownloads:
    """Reject any accidental acquisition during update checking."""

    async def install(
        self,
        offer: ModelArtifactOffer,
        progress: Callable[[int, int], None] | None = None,
    ) -> str:
        """Keep update exploration independent of transfers."""
        raise AssertionError("Update checks must not install artifacts.")


def fixture(
    tmp_path: Path,
) -> tuple[ModelUpdateService, ModelCatalogEntry, UpdateProvider]:
    """Build real isolated persistence and catalog boundaries around tiny stat evidence."""
    (tmp_path / "installed.safetensors").write_bytes(b"fixture")
    catalog = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(
        FixtureFileRoots(tmp_path), catalog, model_relationships()
    ).refresh()
    entry = catalog.list_entries()[0]
    catalog.save_fingerprint(entry.evidence, "b" * 64)
    catalog.save_provider_metadata(
        "b" * 64,
        (
            ProviderMetadataRecord(
                ModelMetadata(
                    "Fixture",
                    base_model="SDXL",
                    provider="civitai",
                    provider_model_id="10",
                    provider_version_id="20",
                )
            ),
        ),
    )
    entry = catalog.list_entries()[0]
    provider = UpdateProvider()
    usage = SqliteModelUsageRepository(tmp_path / "usage.sqlite3")
    return (
        ModelUpdateService(
            usage,
            catalog,
            ModelReleaseService(catalog, provider, NoDownloads(), usage),
            clock=lambda: NOW,
        ),
        entry,
        provider,
    )
