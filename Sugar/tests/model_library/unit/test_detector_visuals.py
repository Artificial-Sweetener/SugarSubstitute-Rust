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
"""Safeguard Substitute's curated detector labels, matching and explicit preference precedence."""

from pathlib import Path

import pytest

from sugarcubes.model_library.domain.detector_visuals import (
    detector_asset,
    detector_title,
    detector_library,
)
from sugarcubes.model_library.application.detector_service import ModelDetectorService
from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.detector_assets import (
    BundledDetectorAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)


@pytest.mark.parametrize(
    ("value", "asset", "title"),
    [
        ("bbox/face_yolov8m.pt", "face-detection", "Face YOLOv8m"),
        ("segm/person_yolov8s-seg.pt", "person-segmentation", "Person YOLOv8s-seg"),
        (
            "Anzhcs Womanface v05 1024 y8n (12.3 MB)",
            "female-face-segmentation",
            "Anzhcs Womanface v05 1024 y8n",
        ),
        (
            "bbox/fuyucchi_yolov8x6_anime_face.pt",
            "anime-face-detection",
            "Fuyucchi YOLOv8x6 anime face",
        ),
        ("unknown.pt", None, "Unknown"),
    ],
)
def test_reference_detector_matching(value: str, asset: str | None, title: str) -> None:
    """Preserve author names, output modes and exact backend values under display enrichment."""
    assert detector_asset(value) == asset
    assert detector_title(value) == title


def test_complete_library_and_durable_exact_associations(tmp_path: Path) -> None:
    """Prepare all reference illustrations once and retain user choices across catalog scans/restart."""
    repository = SqliteModelCatalogRepository(tmp_path)
    assets = ModelThumbnailAssets(repository.asset_root)
    service = ModelDetectorService(repository, BundledDetectorAssets(assets))
    assert len(detector_library()) == 17
    assert len(service.library()) == 17
    before = repository.revision()
    assert service.resolve(("unknown.pt",))[0]["thumbnails"] == []
    service.assign("unknown.pt", "eyes-segmentation")
    assert repository.revision() > before
    repository.reconcile((), None)
    restarted = ModelDetectorService(
        SqliteModelCatalogRepository(tmp_path), BundledDetectorAssets(assets)
    )
    resolved = restarted.resolve(("unknown.pt", "Unknown.pt"))
    assert resolved[0]["value"] == "unknown.pt"
    assert resolved[0]["detectorAsset"] == "eyes-segmentation"
    assert resolved[1]["detectorAsset"] == ""
    thumbnails = resolved[0]["thumbnails"]
    assert isinstance(thumbnails, list)
    assert {image["role"] for image in thumbnails if isinstance(image, dict)} == {
        "standard",
        "banner",
    }
    revision = repository.revision()
    with pytest.raises(ValueError):
        service.assign("unknown.pt", "../../private")
    with pytest.raises(ValueError):
        service.assign(" ", "face-detection")
    assert repository.revision() == revision
