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
"""Prove the host-neutral model catalog preserves selection and prepared assets."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
from sugarcubes.model_library.domain.provider_metadata import ProviderMetadataRecord

from pathlib import Path

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.domain.models import (
    ModelMetadata,
    ThumbnailReference,
)
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.metadata_codec import (
    decode_metadata,
    encode_metadata,
)
from tests.model_library.support.model_roots import ObservedModelRoots


def test_fast_reads_preserve_literal_comfy_value_and_markup_title(
    tmp_path: Path,
) -> None:
    """Avoid scanning or hashing on picker reads and retain text as plain data."""
    roots = ObservedModelRoots()
    repository = SqliteModelCatalogRepository(tmp_path)
    service = ModelCatalogService(roots, repository, model_relationships())
    service.refresh()
    repository.save_fingerprint(roots.evidence, "A" * 64)
    thumbnail = ThumbnailReference("B" * 64, "standard", 128, 192, "image/webp")
    metadata = ModelMetadata("<img src=x onerror=bad>", "v2", thumbnails=(thumbnail,))
    repository.save_provider_metadata("A" * 64, (ProviderMetadataRecord(metadata),))
    result = service.list_models()
    assert result["schemaVersion"] == 1
    models = result["models"]
    assert isinstance(models, list)
    model: object = models[0]
    assert isinstance(model, dict)
    assert model["value"] == roots.evidence.value
    assert model["title"] == metadata.title
    assert model["folder"] == "Anima"
    assert roots.scans == 1
    service.list_models()
    assert roots.scans == 1
    decoded = decode_metadata(encode_metadata(metadata))
    assert decoded.title == metadata.title
    assert decoded.thumbnails[0].asset_id == "b" * 64
