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
"""Isolate public GIMP canvas geometry and selection primitives from application policy."""

from __future__ import annotations

from typing import Protocol

from .image_api import ImageLookup, NativeImage


class GeometryImages(ImageLookup[NativeImage], Protocol):
    """Expose the public image class's unbound resize operation."""

    def resize(
        self, image: NativeImage, width: int, height: int, left: int, top: int
    ) -> bool:
        """Resize bounds and reposition existing layers and channels without resampling."""
        ...


class GeometrySelection(Protocol):
    """Read selection extents without changing selected layers or coverage."""

    def bounds(self, image: NativeImage) -> tuple[bool, bool, int, int, int, int]:
        """Return success, nonempty state and exclusive lower-right bounds."""
        ...


class GimpGeometryApi(Protocol):
    """Keep geometry SDK declarations separate from source and output operations."""

    @property
    def Image(self) -> GeometryImages:
        """Borrow image primitives without replacing the SDK namespace."""
        ...

    @property
    def Selection(self) -> GeometrySelection:
        """Borrow the public selection-bounds reader."""
        ...
