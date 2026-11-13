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
"""Prove local image enrichment is portable, cached, and independent of model loading."""

from __future__ import annotations

from sugarcubes.model_library.infrastructure.relationship_catalog import (
    model_relationships,
)
import json
from pathlib import Path
import pytest

from PIL import Image

from sugarcubes.model_library.application.catalog_service import ModelCatalogService
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.local_presentation import (
    LocalModelPresentation,
)
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.model_roots import FixtureFileRoots
from sugarcubes.model_library.infrastructure.local_metadata_codec import (
    parse_local_model_metadata,
)


def test_local_previews_are_prepared_once_without_asserting_sidecar_hash(
    tmp_path: Path,
) -> None:
    """Keep unverified hash claims out of provider identity and reuse stable prepared assets."""
    model = tmp_path / "portrait.version.safetensors"
    model.write_bytes(b"fixture-only")
    model.with_suffix(".json").write_text(
        json.dumps(
            {
                "model": {"name": "<b>Portrait</b>"},
                "name": "Version 2",
                "sha256": "f" * 64,
                "trainedWords": ["portrait"],
            }
        ),
        encoding="utf-8",
    )
    Image.new("RGB", (240, 360), "#336699").save(model.with_suffix(".preview.png"))
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    catalog = ModelCatalogService(roots, repository, model_relationships())
    catalog.refresh()
    local = LocalModelPresentation(
        roots,
        repository,
        ModelThumbnailPreparer(
            ModelThumbnailAssets(repository.asset_root), origin="local"
        ),
    )
    assert local.refresh(None) == 1
    entry = repository.list_entries()[0]
    assert entry.sha256 is None
    assert entry.metadata is not None
    assert entry.metadata.title == "<b>Portrait</b>"
    assert entry.metadata.version == "Version 2"
    assert len(entry.metadata.thumbnails) == 4
    revision = repository.revision()
    assert local.refresh(None) == 0
    assert repository.revision() == revision
    model.with_suffix(".json").write_text(
        json.dumps({"title": "New presentation"}), encoding="utf-8"
    )
    assert local.refresh(None) == 1
    assert repository.list_entries()[0].metadata is not None


def test_corrupt_local_metadata_does_not_abort_other_entries(tmp_path: Path) -> None:
    """Isolate broken sidecars while leaving valid catalog choices available."""
    model = tmp_path / "broken.safetensors"
    model.write_bytes(b"fixture-only")
    model.with_suffix(".json").write_text("{", encoding="utf-8")
    (tmp_path / "valid.safetensors").write_bytes(b"fixture-only")
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    local = LocalModelPresentation(
        roots,
        repository,
        ModelThumbnailPreparer(
            ModelThumbnailAssets(repository.asset_root), origin="local"
        ),
    )
    assert local.refresh(None) == 1
    assert len(repository.list_entries()) == 2


@pytest.mark.parametrize("suffix", [".json", ".civitai.info", ".info.json"])
def test_local_provider_references_are_available_without_hashing(
    tmp_path: Path, suffix: str
) -> None:
    """Import provider IDs from existing sidecar conventions while leaving hashes unverified."""
    model = tmp_path / "fixture.safetensors"
    model.write_bytes(b"tiny fixture")
    model.with_suffix(suffix).write_text(
        json.dumps({"modelId": "123", "modelVersionId": 456, "sha256": "a" * 64}),
        encoding="utf-8",
    )
    roots = FixtureFileRoots(tmp_path)
    repository = SqliteModelCatalogRepository(tmp_path)
    ModelCatalogService(roots, repository, model_relationships()).refresh()
    local = LocalModelPresentation(
        roots,
        repository,
        ModelThumbnailPreparer(
            ModelThumbnailAssets(repository.asset_root), origin="local"
        ),
    )
    assert local.refresh(None) == 1
    entry = repository.list_entries()[0]
    assert entry.sha256 is None and entry.metadata is not None
    assert entry.metadata.provider == "civitai"
    assert entry.metadata.provider_model_id == "123"
    assert entry.metadata.provider_version_id == "456"
    assert entry.metadata.provider_url == "https://civitai.com/models/123"
    assert local.refresh(None) == 0


def test_local_version_shape_and_malformed_provider_hints() -> None:
    """Accept CivitAI's saved version shape and keep arbitrary IDs and URLs out of transport."""
    metadata = parse_local_model_metadata(
        {"id": 20, "modelId": 10, "model": {"name": "Fixture"}, "name": "v2"},
        "fallback",
        (),
    )
    assert metadata.provider_version_id == "20" and metadata.version == "v2"
    for invalid in (True, -1, "0", "../20", "9" * 21, "https://127.0.0.1/private"):
        metadata = parse_local_model_metadata(
            {
                "modelId": invalid,
                "modelVersionId": 20,
                "providerUrl": "https://127.0.0.1/private",
            },
            "fallback",
            (),
        )
        assert metadata.provider == "" and metadata.provider_url == ""
        assert metadata.provider_model_id == "" and metadata.provider_version_id == ""
