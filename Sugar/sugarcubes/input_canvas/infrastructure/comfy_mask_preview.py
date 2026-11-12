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
"""Decode native effective mask coverage for bounded previews and full-resolution execution snapshots."""

from __future__ import annotations

from collections.abc import Sequence
from importlib import import_module
from io import BytesIO
from typing import Protocol, cast

from .input_preview_settings import InputPreviewSettings
from .comfy_media_path import validate_input_file
from .comfy_mask_loader import load_native_mask

import numpy as np
import numpy.typing as npt
from PIL import Image


class _NativeMaskTensor(Protocol):
    """Read CPU mask samples without spreading dynamic tensor APIs into application code."""

    @property
    def shape(self) -> Sequence[int]:
        """Expose native batch, height and width."""

    def detach(self) -> _NativeMaskTensor:
        """Retain a read-only detached view for preview conversion."""

    def cpu(self) -> _NativeMaskTensor:
        """Access decoded image samples on the CPU."""

    def numpy(self) -> npt.NDArray[np.float32]:
        """Expose native floating-point coverage."""

    def float(self) -> _NativeMaskTensor:
        """Convert native intermediate precision into numpy-compatible coverage."""


def native_mask_preview(
    file: str, settings: InputPreviewSettings, maximum_size: int
) -> bytes:
    """Return native grayscale coverage; a zero maximum preserves its full resolution."""
    if not 0 <= maximum_size <= 1024:
        raise ValueError("Invalid mask preview size.")
    validate_input_file(file)
    tensor_value = load_native_mask(file, settings)
    tensor_type: object = getattr(import_module("torch"), "Tensor", None)
    if not isinstance(tensor_type, type) or not isinstance(tensor_value, tensor_type):
        raise RuntimeError("Comfy returned an unsupported mask value.")
    tensor = cast(_NativeMaskTensor, tensor_value)
    shape = tuple(tensor.shape)
    if (
        len(shape) != 3
        or any(size < 1 for size in shape)
        or shape[1] * shape[2] > 64 * 1024 * 1024
    ):
        raise ValueError("The effective mask exceeds the supported preview size.")
    if maximum_size == 0 and shape[0] != 1:
        raise ValueError("Input editing requires one effective mask.")
    values = tensor.detach().cpu().float().numpy()
    if not np.isfinite(values).all():
        raise ValueError("The effective mask contains invalid coverage.")
    coverage = np.rint(np.clip(values[0], 0, 1) * 255).astype(np.uint8)
    image = Image.fromarray(coverage)
    if maximum_size:
        image.thumbnail((maximum_size, maximum_size), Image.Resampling.BILINEAR)
    output = BytesIO()
    image.save(output, format="PNG")
    return output.getvalue()
