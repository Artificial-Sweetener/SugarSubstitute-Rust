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
"""Exclude native channel overlays and display-only component filtering from captured image pixels."""

from __future__ import annotations

from .image_api import GimpImageApi, NativeImage
from .model import NativeOperationUnsupported


def prepare_snapshot_projection(gimp: GimpImageApi, image: NativeImage) -> None:
    """Normalize an owned copy for canonical RGB capture without changing its source."""
    for channel in image.get_channels():
        if channel.get_visible() and not channel.set_visible(False):
            raise RuntimeError(
                "GIMP could not suppress a frozen channel display overlay."
            )
    base = image.get_base_type()
    if base in (gimp.ImageBaseType.GRAY, gimp.ImageBaseType.INDEXED):
        # GIMP's public PDB admits alpha visibility changes only for RGB images.
        if not image.convert_rgb():
            raise RuntimeError("GIMP could not normalize the frozen image to RGB.")
    elif base != gimp.ImageBaseType.RGB:
        raise NativeOperationUnsupported("This GIMP color model cannot be captured.")
    components = gimp.ChannelType
    for component in (
        components.RED,
        components.GREEN,
        components.BLUE,
        components.ALPHA,
    ):
        if not image.set_component_visible(component, True):
            raise RuntimeError("GIMP could not prepare frozen image color components.")
