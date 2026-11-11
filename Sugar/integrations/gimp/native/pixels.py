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
"""Read bounded native GEGL pixels without interpreting their workflow meaning."""

from __future__ import annotations

from math import floor, isfinite
from typing import Literal

from .image_api import NativeDrawable
from .model import NativeDocument, NativePixels
from .sdk import GeglApi


class NativePixelReader:
    """Apply the shared adapter's admitted byte budget at the native allocation boundary."""

    def __init__(self, gegl: GeglApi, maximum_bytes: int) -> None:
        """Receive the transfer budget from the shared contract's composition owner."""
        if maximum_bytes < 1:
            raise ValueError("The native pixel budget must be positive.")
        self._gegl = gegl
        self._maximum_bytes = maximum_bytes

    def read(
        self,
        drawable: NativeDrawable,
        document: NativeDocument,
        width: int,
        height: int,
        scale: float,
        format_name: Literal["rgba8", "coverage8"],
    ) -> NativePixels:
        """Read full-canvas pixels with the exact scale selected by the shared adapter."""
        return self._read(
            drawable,
            document,
            width,
            height,
            scale,
            format_name,
            "R'G'B'A u8" if format_name == "rgba8" else "Y u8",
        )

    def read_channel(
        self,
        drawable: NativeDrawable,
        document: NativeDocument,
        width: int,
        height: int,
        scale: float,
        nonlinear: bool,
    ) -> NativePixels:
        """Match GIMP's raw-copy channel exception instead of gamma-converting coverage samples."""
        return self._read(
            drawable,
            document,
            width,
            height,
            scale,
            "coverage8",
            "Y' u8" if nonlinear else "Y u8",
        )

    def _read(
        self,
        drawable: NativeDrawable,
        document: NativeDocument,
        width: int,
        height: int,
        scale: float,
        format_name: Literal["rgba8", "coverage8"],
        native_format: str,
    ) -> NativePixels:
        """Read an admitted full-canvas region using its native color or coverage interpretation."""
        size = self.admit(document, width, height, scale, format_name)
        rectangle = self._gegl.Rectangle.new(0, 0, width, height)
        data = drawable.get_buffer().get(
            rectangle,
            scale,
            native_format,
            self._gegl.AbyssPolicy.CLAMP,
        )
        if len(data) != size:
            raise RuntimeError(
                "GIMP returned a different pixel byte count than requested."
            )
        return NativePixels(width, height, format_name, data)

    def admit(
        self,
        document: NativeDocument,
        width: int,
        height: int,
        scale: float,
        format_name: Literal["rgba8", "coverage8"],
    ) -> int:
        """Admit geometry before a native projection or output buffer can be allocated."""
        if not isfinite(scale) or scale <= 0 or scale > 1:
            raise ValueError("Invalid native capture scale.")
        if width != max(1, floor(document.width * scale + 0.5)) or height != max(
            1, floor(document.height * scale + 0.5)
        ):
            raise ValueError(
                "Native capture dimensions disagree with its canvas scale."
            )
        size = width * height * (4 if format_name == "rgba8" else 1)
        if size > self._maximum_bytes:
            raise ValueError("The native capture exceeds its admitted pixel budget.")
        return size
