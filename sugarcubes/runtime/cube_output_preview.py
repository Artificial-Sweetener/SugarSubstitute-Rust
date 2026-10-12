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
"""Coordinate image persistence and best-effort video previews for Cube outputs."""

from __future__ import annotations

import logging

from .cube_output_events import CubeOutputArtifact
from .cube_output_images import build_image_output_preview
from .video_output_preview import build_video_output_preview

_LOGGER = logging.getLogger(__name__)


def build_cube_output_preview(
    value: object,
    *,
    save_outputs: bool = False,
) -> tuple[dict[str, object], tuple[CubeOutputArtifact, ...]]:
    """Preserve image-save failures while keeping video preview failures nonblocking."""

    images = build_image_output_preview(value, save_outputs=save_outputs)
    if images[1]:
        return images
    try:
        return build_video_output_preview(value)
    except (
        AttributeError,
        ImportError,
        ModuleNotFoundError,
        OSError,
        TypeError,
        ValueError,
    ):
        _LOGGER.exception(
            "Failed to create cube video output preview artifact.",
            extra={"value_type": type(value).__qualname__},
        )
        return {}, ()
