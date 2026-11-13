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
"""Prove only model values create host-addressable model artifacts."""

from __future__ import annotations

from pathlib import Path
import sys
from types import ModuleType

import pytest
import numpy as np
from numpy.typing import NDArray

from sugarcubes.runtime.model_output_preview import build_model_output_preview


class ModelFile:
    """Represent an encoded synthetic model at the external host boundary."""

    def __init__(self, format: str = "glb") -> None:
        """Retain a declared synthetic format."""
        self.format = format

    def save_to(self, path: str) -> None:
        """Materialize only deterministic fixture bytes."""
        Path(path).write_bytes(b"synthetic-model")


def test_encoded_model_uses_configured_storage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Preserve model type and respect the Cube's persistence intent."""
    storage = ModuleType("folder_paths")
    setattr(storage, "get_temp_directory", lambda: str(tmp_path / "temp"))
    setattr(storage, "get_output_directory", lambda: str(tmp_path / "output"))
    monkeypatch.setitem(sys.modules, "folder_paths", storage)
    for save_outputs in (False, True):
        payload, artifacts = build_model_output_preview(
            ModelFile(), save_outputs=save_outputs
        )
        kind = "output" if save_outputs else "temp"
        assert artifacts[0].media_kind == "model"
        assert artifacts[0].type == kind
        assert (
            tmp_path / kind / artifacts[0].filename
        ).read_bytes() == b"synthetic-model"
        assert payload == {
            "3d": [{"filename": artifacts[0].filename, "subfolder": "", "type": kind}]
        }


@pytest.mark.parametrize(
    "value", ["personal.glb", object(), ModelFile("../glb"), ModelFile("mp4")]
)
def test_unqualified_model_values_produce_no_artifacts(value: object) -> None:
    """Reject arbitrary paths and formats before consulting storage or host writers."""
    assert build_model_output_preview(value) == ({}, ())


class ModelTensor:
    """Represent only the external tensor operations needed for export qualification."""

    def __init__(self, shape: tuple[int, ...]) -> None:
        """Store synthetic tensor geometry."""
        self.shape = shape

    def __getitem__(self, index: int) -> ModelTensor:
        """Select a deterministic texture or mesh entry."""
        return ModelTensor(self.shape[1:])

    def clamp(self, minimum: float, maximum: float) -> ModelTensor:
        """Return already-bounded synthetic samples."""
        return self

    def cpu(self) -> ModelTensor:
        """Keep fixture samples in CPU memory."""
        return self

    def numpy(self) -> NDArray[np.generic]:
        """Produce original fixture pixels without reading image files."""
        return np.zeros(self.shape, dtype=np.uint8)


class ModelMesh:
    """Describe two textured synthetic mesh batches at the host boundary."""

    vertices = ModelTensor((2, 3, 3))
    faces = ModelTensor((2, 1, 3))
    texture = ModelTensor((2, 2, 2, 3))
    unlit = True


def test_mesh_batches_delegate_geometry_and_material_export_to_comfy(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Preserve batch slicing, UVs, colors and texture through the host writer."""
    from PIL import Image

    storage = ModuleType("folder_paths")
    setattr(storage, "get_temp_directory", lambda: str(tmp_path))
    monkeypatch.setitem(sys.modules, "folder_paths", storage)
    host = ModuleType("comfy_extras.nodes_save_3d")
    slices: list[int] = []
    writes: list[Path] = []
    points = ModelTensor((3, 3))
    triangles = ModelTensor((1, 3))
    colors = ModelTensor((3, 3))
    uvs = ModelTensor((3, 2))

    def slice_mesh(
        mesh: object, index: int
    ) -> tuple[ModelTensor, ModelTensor, ModelTensor, ModelTensor]:
        """Record the canonical host batch-slicing call."""
        assert isinstance(mesh, ModelMesh)
        slices.append(index)
        return points, triangles, colors, uvs

    def write_mesh(
        vertices: object,
        faces: object,
        filepath: str,
        *,
        uvs: object,
        vertex_colors: object,
        texture_image: Image.Image,
        unlit: bool,
    ) -> None:
        """Record material-preserving serialization at the external host boundary."""
        assert vertices is points and faces is triangles
        assert uvs is not None and vertex_colors is colors
        assert texture_image.size == (2, 2) and unlit
        path = Path(filepath)
        path.write_bytes(b"synthetic-host-glb")
        writes.append(path)

    setattr(host, "get_mesh_batch_item", slice_mesh)
    setattr(host, "save_glb", write_mesh)
    monkeypatch.setitem(sys.modules, "comfy_extras.nodes_save_3d", host)
    _, artifacts = build_model_output_preview(ModelMesh())
    assert slices == [0, 1]
    assert len(artifacts) == len(writes) == 2
    assert all(
        artifact.type == "temp" and artifact.media_kind == "model"
        for artifact in artifacts
    )
