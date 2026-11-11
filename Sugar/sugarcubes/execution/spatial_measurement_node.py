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
"""Measure ephemeral Comfy tensor frames without producing previews or files."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import TypedDict


class SpatialMeasurementUi(TypedDict):
    """Carry only two measured frame sizes through native execution history."""

    spatial_dimensions: list[list[int]]


class SpatialMeasurementResult(TypedDict):
    """Publish measurements without exposing ordinary generation media keys."""

    ui: SpatialMeasurementUi
    result: tuple[()]


class SpatialProbeMeasurementNode:
    """Act as the sole execution sink in a bounded spatial probe."""

    RETURN_TYPES: tuple[()] = ()
    FUNCTION = "measure"
    OUTPUT_NODE = True
    CATEGORY = "_SugarCubes/Internal"

    @classmethod
    def INPUT_TYPES(cls) -> dict[str, dict[str, tuple[str]]]:
        """Accept either a matched image pair or a matched latent pair."""
        return {
            "optional": {
                "image_a": ("IMAGE",),
                "image_b": ("IMAGE",),
                "latent_a": ("LATENT",),
                "latent_b": ("LATENT",),
            }
        }

    def measure(
        self,
        image_a: object = None,
        image_b: object = None,
        latent_a: object = None,
        latent_b: object = None,
    ) -> SpatialMeasurementResult:
        """Return only shape metadata and reject malformed or excessively large native frames."""
        images = image_a is not None and image_b is not None
        latents = latent_a is not None and latent_b is not None
        if images == latents:
            raise ValueError("A spatial probe requires exactly one matched frame pair.")
        sizes: list[list[int]] = []
        for value in ((image_a, image_b) if images else (latent_a, latent_b)):
            if latents:
                if not isinstance(value, Mapping):
                    raise ValueError("The measured latent is unavailable.")
                value = value.get("samples")
            shape: object = getattr(value, "shape", None)
            if not isinstance(shape, Sequence) or len(shape) != 4:
                raise ValueError("The measured frame has an unsupported tensor shape.")
            width, height = (shape[2], shape[1]) if images else (shape[3], shape[2])
            if (
                type(width) is not int
                or type(height) is not int
                or width <= 0
                or height <= 0
                or width * height > 16777216
            ):
                raise ValueError("The measured frame exceeds its pixel budget.")
            sizes.append([width, height])
        return {"ui": {"spatial_dimensions": sizes}, "result": ()}
