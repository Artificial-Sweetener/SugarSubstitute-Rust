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
"""A dropped generated image brings back its run's saved images from its own folder."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest
from PIL import Image
from PIL.PngImagePlugin import PngInfo

from sugarcubes.output_saving.infrastructure.artifact_registry import (
    ManagedArtifactRegistry,
)
from sugarcubes.output_saving.infrastructure.run_images import (
    DroppedImage,
    OutputRunImages,
    read_dropped_image,
)


def _save(
    registry: ManagedArtifactRegistry,
    root: Path,
    relative: str,
    run_id: str,
    source: str,
    colour: int,
) -> Path:
    """Write and register one saved output carrying its run record."""
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    info = PngInfo()
    info.add_text(
        "sugarcubes_output",
        json.dumps(
            {
                "schema_version": 1,
                "run_id": run_id,
                "source_node_id": source,
                "source_label": source.split(":")[-1],
                "image_index": 1,
            }
        ),
    )
    Image.new("RGB", (8, 8), (colour, 0, 0)).save(path, pnginfo=info)
    registry.register(root=root, png=path, jpeg=None, saved=True, width=8, height=8)
    return path


def _dropped(path: Path, run_id: str) -> DroppedImage:
    """Describe a file as the page does once it is dropped."""
    data = path.read_bytes()
    return DroppedImage(path.name, len(data), hashlib.sha256(data).hexdigest(), run_id)


def test_a_dropped_image_lists_its_runs_images_in_its_own_folder(
    tmp_path: Path,
) -> None:
    """Images of the same run in that folder come back in name order; nothing else does."""
    registry = ManagedArtifactRegistry(tmp_path / "artifacts.sqlite")
    root = tmp_path / "outputs"
    first = _save(registry, root, "day/001_01_t2i.png", "run-a", "cube-a:0", 10)
    _save(registry, root, "day/001_02_upscale.png", "run-a", "cube-b:0", 20)
    _save(registry, root, "day/002_01_t2i.png", "run-b", "cube-a:0", 30)
    _save(registry, root, "other/001_03_detail.png", "run-a", "cube-c:0", 40)

    images = OutputRunImages(registry).find(_dropped(first, "run-a"))

    assert [image.image["filename"] for image in images] == [
        "001_01_t2i.png",
        "001_02_upscale.png",
    ]
    assert [image.source_node_id for image in images] == ["cube-a:0", "cube-b:0"]
    assert all(
        str(image.image["subfolder"]).startswith("__sugarcubes_managed__/")
        for image in images
    )


def test_an_image_not_saved_here_brings_back_nothing(tmp_path: Path) -> None:
    """A file with the same name but other bytes is not the saved image."""
    registry = ManagedArtifactRegistry(tmp_path / "artifacts.sqlite")
    root = tmp_path / "outputs"
    saved = _save(registry, root, "day/001_01_t2i.png", "run-a", "cube-a:0", 10)
    elsewhere = tmp_path / "downloads" / saved.name
    elsewhere.parent.mkdir()
    Image.new("RGB", (8, 8), (99, 99, 99)).save(elsewhere)

    assert OutputRunImages(registry).find(_dropped(elsewhere, "run-a")) == ()


@pytest.mark.parametrize(
    "value",
    [
        {"filename": "../x.png", "size": 1, "sha256": "a" * 64, "run_id": "r"},
        {"filename": "x.png", "size": 0, "sha256": "a" * 64, "run_id": "r"},
        {"filename": "x.png", "size": 1, "sha256": "short", "run_id": "r"},
        {"filename": "x.png", "size": 1, "sha256": "a" * 64, "run_id": " "},
        [],
    ],
)
def test_a_malformed_description_is_refused(value: object) -> None:
    """The route admits only a plain file name, a positive size, a full hash and a run."""
    with pytest.raises(ValueError):
        read_dropped_image(value)
