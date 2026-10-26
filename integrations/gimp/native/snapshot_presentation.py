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
    """Normalize only an owned copy because GIMP's image projection includes native display overlays."""
    for channel in image.get_channels():
        if channel.get_visible() and not channel.set_visible(False):
            raise RuntimeError(
                "GIMP could not suppress a frozen channel display overlay."
            )
    base = image.get_base_type()
    components = gimp.ChannelType
    visible: tuple[int, ...]
    if base == gimp.ImageBaseType.RGB:
        visible = (components.RED, components.GREEN, components.BLUE, components.ALPHA)
    elif base == gimp.ImageBaseType.GRAY:
        visible = (components.GRAY, components.ALPHA)
    elif base == gimp.ImageBaseType.INDEXED:
        visible = (components.INDEXED, components.ALPHA)
    else:
        raise NativeOperationUnsupported("This GIMP color model cannot be captured.")
    for component in visible:
        if not image.set_component_visible(component, True):
            raise RuntimeError("GIMP could not prepare frozen image color components.")
