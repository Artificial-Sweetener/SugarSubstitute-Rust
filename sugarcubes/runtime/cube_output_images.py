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
"""Materialize Cube image outputs through Comfy file conventions and explicit persistence policy."""

from __future__ import annotations

import logging
import os
import random
from collections.abc import Iterator, Sequence
from typing import Literal, Protocol, TypeGuard, runtime_checkable

import numpy as np
from numpy.typing import NDArray

from . import CubeOutputArtifact

_logger = logging.getLogger(__name__)


def build_cube_output_preview(
    value: object,
    *,
    save_outputs: bool = False,
) -> tuple[dict[str, object], tuple[CubeOutputArtifact, ...]]:
    """Create image artifacts under the workflow instance output policy."""

    if not _looks_like_image_batch(value):
        return {}, ()
    try:
        return _save_preview_images(value, save_outputs=save_outputs)
    except (
        AttributeError,
        ImportError,
        ModuleNotFoundError,
        OSError,
        TypeError,
        ValueError,
    ):
        _logger.exception(
            "Failed to create cube output preview artifact.",
            extra={"value_type": type(value).__qualname__},
        )
        if save_outputs:
            raise
        return {}, ()


def _looks_like_image_batch(value: object) -> TypeGuard[_ImageBatch]:
    """Return whether a value resembles a Comfy IMAGE tensor batch."""

    if not isinstance(value, _ImageBatch):
        return False
    shape = value.shape
    if len(shape) != 4:
        return False
    try:
        batch, height, width, channels = shape
    except (TypeError, ValueError):
        return False
    return (
        int(batch) > 0
        and int(height) > 0
        and int(width) > 0
        and int(channels) in {1, 3, 4}
    )


def _save_preview_images(
    images: _ImageBatch,
    *,
    save_outputs: bool,
) -> tuple[dict[str, object], tuple[CubeOutputArtifact, ...]]:
    """Save images through Comfy's output or temporary image conventions."""

    import folder_paths
    from comfy.cli_args import args
    from PIL import Image
    from PIL.PngImagePlugin import PngInfo

    output_dir = (
        folder_paths.get_output_directory()
        if save_outputs
        else folder_paths.get_temp_directory()
    )
    output_type: Literal["temp", "output"] = "output" if save_outputs else "temp"
    filename_prefix = ("SugarCubes_" if save_outputs else "ComfyUI_temp_") + "".join(
        random.choice("abcdefghijklmnopqrstupvxyz") for _ in range(5)
    )
    full_output_folder, filename, counter, subfolder, _ = (
        folder_paths.get_save_image_path(
            filename_prefix,
            output_dir,
            images[0].shape[1],
            images[0].shape[0],
        )
    )
    ui_images: list[dict[str, object]] = []
    artifacts: list[CubeOutputArtifact] = []
    for batch_number, image in enumerate(images):
        image_array = _image_to_numpy(image)
        scaled_image = np.multiply(image_array, 255.0)
        pil_image = Image.fromarray(np.clip(scaled_image, 0, 255).astype(np.uint8))
        metadata = None if args.disable_metadata else PngInfo()
        filename_with_batch_num = filename.replace("%batch_num%", str(batch_number))
        file_name = f"{filename_with_batch_num}_{counter:05}_.png"
        pil_image.save(
            os.path.join(full_output_folder, file_name),
            pnginfo=metadata,
            compress_level=1,
        )
        image_ref: dict[str, object] = {
            "filename": file_name,
            "subfolder": subfolder,
            "type": output_type,
        }
        ui_images.append(image_ref)
        artifacts.append(
            CubeOutputArtifact(
                filename=file_name,
                subfolder=subfolder,
                type=output_type,
                media_kind="image",
                mime_type="image/png",
                width=pil_image.width,
                height=pil_image.height,
            )
        )
        counter += 1
    return {"images": ui_images}, tuple(artifacts)


def _image_to_numpy(image: _ImageItem) -> NDArray[np.generic]:
    """Convert a Comfy image tensor item to a NumPy-compatible array."""

    return np.asarray(image.cpu().numpy())


@runtime_checkable
class _ImageItem(Protocol):
    """Describe one tensor-like image item at the Comfy adapter boundary."""

    shape: Sequence[int]

    def cpu(self) -> _ImageItem:
        """Return a CPU-backed tensor view."""

    def numpy(self) -> object:
        """Return the tensor's array representation."""


@runtime_checkable
class _ImageBatch(Protocol):
    """Describe the tensor batch surface consumed by preview generation."""

    shape: Sequence[int]

    def __iter__(self) -> Iterator[_ImageItem]:
        """Iterate image items."""

    def __getitem__(self, index: int) -> _ImageItem:
        """Return one image item."""
