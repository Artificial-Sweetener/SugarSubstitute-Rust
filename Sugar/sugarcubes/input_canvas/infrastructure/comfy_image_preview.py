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
"""Decode an upload field's actual source image through Comfy without executing its node's processing graph."""

from __future__ import annotations
from collections.abc import Sequence
from importlib import import_module
from io import BytesIO
from typing import Protocol, cast
import numpy as np
import numpy.typing as npt
from PIL import Image
from .comfy_media_path import validate_input_file
from .input_preview_settings import InputPreviewSettings


class _ImageLoader(Protocol):
    """Describe Comfy's source-file codec at the native boundary."""

    def load_image(self, image: str) -> object:
        """Decode image colors and native inverse-alpha coverage."""


class _Tensor(Protocol):
    """Read the native source tensors while retaining exact geometry and intermediate precision."""

    @property
    def shape(self) -> Sequence[int]:
        """Expose native batch and frame geometry."""

    def detach(self) -> _Tensor:
        """Retain a read-only source value."""

    def cpu(self) -> _Tensor:
        """Move decoded samples to the preview worker."""

    def float(self) -> _Tensor:
        """Normalize native intermediate precision for numpy."""

    def numpy(self) -> npt.NDArray[np.float32]:
        """Expose normalized source samples."""


def native_image_preview(
    file: str, settings: InputPreviewSettings, maximum_size: int
) -> bytes:
    """Read source pixels at display or full resolution while node operations remain in the workflow."""
    if settings.kind != "image" or not 0 <= maximum_size <= 1024:
        raise ValueError("Invalid image source preview settings.")
    validate_input_file(file)
    nodes = import_module("nodes")
    mapping: object = getattr(nodes, "NODE_CLASS_MAPPINGS", None)
    constructor: object = (
        mapping.get(settings.class_type) if isinstance(mapping, dict) else None
    )
    schema_method: object = getattr(constructor, "INPUT_TYPES", None)
    schema: object = schema_method() if callable(schema_method) else None
    definition: object = None
    if isinstance(schema, dict):
        for section in ("required", "optional"):
            fields: object = schema.get(section, {})
            if isinstance(fields, dict) and settings.field in fields:
                definition = fields[settings.field]
    metadata: object = (
        definition[1]
        if isinstance(definition, (tuple, list)) and len(definition) > 1
        else None
    )
    if (
        not isinstance(metadata, dict)
        or metadata.get("image_upload") is not True
        or metadata.get("image_folder") not in (None, "input", "output", "temp")
    ):
        raise ValueError("The selected field is not a registered image upload source.")
    core: object = mapping.get("LoadImage") if isinstance(mapping, dict) else None
    loader: object = core() if callable(core) else None
    if not callable(getattr(loader, "load_image", None)):
        raise RuntimeError("Comfy's source image decoder is unavailable.")
    result = cast(_ImageLoader, loader).load_image(file)
    if not isinstance(result, (tuple, list)) or len(result) < 2:
        raise RuntimeError("Comfy returned no decoded source image.")
    image_tensor = _tensor(result[0])
    mask_tensor = _tensor(result[1])
    shape = tuple(image_tensor.shape)
    if (
        len(shape) != 4
        or any(size < 1 for size in shape)
        or shape[3] != 3
        or shape[1] * shape[2] * 4 > 64 * 1024 * 1024
    ):
        raise ValueError("The image source exceeds the supported preview size.")
    if maximum_size == 0 and shape[0] != 1:
        raise ValueError("Input editing requires one decoded source image.")
    colors = image_tensor.detach().cpu().float().numpy()[0]
    if not np.isfinite(colors).all():
        raise ValueError("The image source contains invalid samples.")
    rgba = np.zeros((shape[1], shape[2], 4), dtype=np.uint8)
    rgba[..., :3] = np.rint(np.clip(colors, 0, 1) * 255).astype(np.uint8)
    rgba[..., 3] = 255
    mask_shape = tuple(mask_tensor.shape)
    if len(mask_shape) != 3 or any(size < 1 for size in mask_shape):
        raise RuntimeError("Comfy returned invalid source alpha coverage.")
    # Native LoadImage uses a 64x64 empty mask when the source file has no alpha channel.
    if mask_shape[1:] == shape[1:3]:
        coverage = mask_tensor.detach().cpu().float().numpy()[0]
        if not np.isfinite(coverage).all():
            raise ValueError("The image source contains invalid alpha.")
        rgba[..., 3] = np.rint((1 - np.clip(coverage, 0, 1)) * 255).astype(np.uint8)
    image = Image.fromarray(rgba)
    if maximum_size:
        image.thumbnail((maximum_size, maximum_size), Image.Resampling.BILINEAR)
    output = BytesIO()
    image.save(output, format="PNG")
    return output.getvalue()


def _tensor(value: object) -> _Tensor:
    """Validate dynamic native values before reading image samples."""
    tensor_type: object = getattr(import_module("torch"), "Tensor", None)
    if not isinstance(tensor_type, type) or not isinstance(value, tensor_type):
        raise RuntimeError("Comfy returned an unsupported source image tensor.")
    return cast(_Tensor, value)
