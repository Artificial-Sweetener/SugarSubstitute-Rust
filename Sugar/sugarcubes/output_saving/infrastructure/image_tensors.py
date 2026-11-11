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
"""Decode one Comfy image tensor at a time without retaining whole CPU batches."""

from __future__ import annotations

from collections.abc import Iterator, Sequence
from typing import Protocol, TypeGuard, runtime_checkable

import numpy as np
from PIL import Image


@runtime_checkable
class ImageTensorItem(Protocol):
    """Describe only the tensor operations required at the host image boundary."""

    shape: Sequence[int]

    def cpu(self) -> ImageTensorItem:
        """Expose this image's CPU pixels without transferring unrelated batch items."""

    def numpy(self) -> object:
        """Expose array data for bounded per-image conversion."""


@runtime_checkable
class ImageTensorBatch(Protocol):
    """Describe Comfy's batch-height-width-channel image shape and item access."""

    shape: Sequence[int]

    def __iter__(self) -> Iterator[ImageTensorItem]:
        """Yield individual image tensors."""

    def __getitem__(self, index: int) -> ImageTensorItem:
        """Expose image geometry without transferring the batch."""


def is_image_tensor_batch(value: object) -> TypeGuard[ImageTensorBatch]:
    """Exclude masks, opaque resources and empty values before image encoding."""
    if not isinstance(value, ImageTensorBatch) or len(value.shape) != 4:
        return False
    batch, height, width, channels = value.shape
    return batch > 0 and height > 0 and width > 0 and channels in {1, 3, 4}


def iter_tensor_images(images: ImageTensorBatch) -> Iterator[Image.Image]:
    """Preserve grayscale and alpha while bounding temporary arrays to one image."""
    for item in images:
        array = np.asarray(item.cpu().numpy())
        pixels = np.clip(np.multiply(array, 255.0), 0, 255).astype(np.uint8)
        if pixels.ndim != 3 or pixels.shape[-1] not in {1, 3, 4}:
            raise ValueError(
                "Comfy image tensor item must contain grayscale, RGB or RGBA pixels."
            )
        if pixels.shape[-1] == 1:
            pixels = pixels[..., 0]
        yield Image.fromarray(pixels)
