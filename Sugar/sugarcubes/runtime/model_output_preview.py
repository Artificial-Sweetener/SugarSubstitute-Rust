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
"""Adapt Comfy model values into artifacts using Comfy's own model writer."""

from __future__ import annotations

import importlib
from pathlib import Path
from typing import Protocol, cast, runtime_checkable
import uuid

from PIL import Image
import numpy as np
from numpy.typing import NDArray

from .cube_output_events import CubeOutputArtifact

_FORMATS = frozenset(
    {"glb", "gltf", "obj", "fbx", "stl", "ply", "splat", "spz", "ksplat"}
)


@runtime_checkable
class _ModelTensor(Protocol):
    """Describe only host tensor operations needed for mesh export."""

    @property
    def shape(self) -> tuple[int, ...]:
        """Return tensor dimensions."""

    def __getitem__(self, index: int) -> _ModelTensor:
        """Select one mesh batch entry."""

    def clamp(self, minimum: float, maximum: float) -> _ModelTensor:
        """Bound texture samples for image encoding."""

    def cpu(self) -> _ModelTensor:
        """Transfer a texture to the host-owned CPU representation."""

    def numpy(self) -> NDArray[np.generic]:
        """Expose texture samples at the image adapter boundary."""


@runtime_checkable
class _ModelFile(Protocol):
    """Describe Comfy's encoded model-file value without importing optional host modules."""

    @property
    def format(self) -> str:
        """Return the encoded model format."""

    def save_to(self, path: str) -> object:
        """Persist the model using its host-owned encoding."""


class _Storage(Protocol):
    """Provide only Comfy's configured artifact roots."""

    def get_output_directory(self) -> str:
        """Return persistent output storage."""

    def get_temp_directory(self) -> str:
        """Return ephemeral output storage."""


class _MeshWriter(Protocol):
    """Isolate the dynamic core mesh writer behind a narrow typed contract."""

    def get_mesh_batch_item(
        self, mesh: object, index: int
    ) -> tuple[_ModelTensor, _ModelTensor, _ModelTensor | None, _ModelTensor | None]:
        """Slice one mesh while preserving variable batch lengths and attributes."""

    def save_glb(
        self,
        vertices: _ModelTensor,
        faces: _ModelTensor,
        filepath: str,
        *,
        uvs: _ModelTensor | None,
        vertex_colors: _ModelTensor | None,
        texture_image: Image.Image | None,
        unlit: bool,
    ) -> None:
        """Write one mesh through Comfy's canonical GLB serializer."""


def build_model_output_preview(
    value: object, *, save_outputs: bool = False
) -> tuple[dict[str, object], tuple[CubeOutputArtifact, ...]]:
    """Materialize validated model values without interpreting arbitrary string paths."""
    declared_format = getattr(value, "format", None)
    extension = declared_format.lower() if isinstance(declared_format, str) else ""
    encoded = isinstance(value, _ModelFile) and extension in _FORMATS
    vertices = getattr(value, "vertices", None)
    faces = getattr(value, "faces", None)
    mesh = (
        isinstance(vertices, _ModelTensor)
        and isinstance(faces, _ModelTensor)
        and len(vertices.shape) == 3
        and len(faces.shape) == 3
    )
    if not encoded and not mesh:
        return {}, ()
    storage = cast(_Storage, importlib.import_module("folder_paths"))
    root = Path(
        storage.get_output_directory() if save_outputs else storage.get_temp_directory()
    )
    root.mkdir(parents=True, exist_ok=True)
    artifact_type = "output" if save_outputs else "temp"
    artifacts: list[CubeOutputArtifact] = []
    if isinstance(value, _ModelFile) and encoded:
        filename = f"SugarCubes_model_{uuid.uuid4().hex}.{extension}"
        value.save_to(str(root / filename))
        artifacts.append(CubeOutputArtifact(filename, "", artifact_type, "model"))
    elif isinstance(vertices, _ModelTensor):
        writer = cast(
            _MeshWriter, importlib.import_module("comfy_extras.nodes_save_3d")
        )
        texture = getattr(value, "texture", None)
        unlit = getattr(value, "unlit", False) is True
        for index in range(vertices.shape[0]):
            points, triangles, colors, uvs = writer.get_mesh_batch_item(value, index)
            if points.shape[0] == 0 or triangles.shape[0] == 0:
                continue
            image = (
                Image.fromarray(
                    (texture[index].clamp(0.0, 1.0).cpu().numpy() * 255).astype("uint8")
                )
                if isinstance(texture, _ModelTensor)
                else None
            )
            filename = f"SugarCubes_model_{uuid.uuid4().hex}.glb"
            writer.save_glb(
                points,
                triangles,
                str(root / filename),
                uvs=uvs,
                vertex_colors=colors,
                texture_image=image,
                unlit=unlit,
            )
            artifacts.append(CubeOutputArtifact(filename, "", artifact_type, "model"))
    return {
        "3d": [
            {
                "filename": artifact.filename,
                "subfolder": artifact.subfolder,
                "type": artifact.type,
            }
            for artifact in artifacts
        ]
    }, tuple(artifacts)
