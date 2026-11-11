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
"""Encode shareable JPEG companions while preserving canonical image geometry."""

from __future__ import annotations

from io import BytesIO

from PIL import Image

from ..domain.preferences import JpegPreferences, JpegSizingMode


class JpegCompanionEncoder:
    """Own RGB conversion and bounded quality search for optional JPEG companions."""

    def encode(self, image: Image.Image, preferences: JpegPreferences) -> bytes:
        """Apply the selected compression policy without scaling or copying recipe metadata."""
        prepared = self._prepare(image)
        if preferences.sizing_mode is JpegSizingMode.QUALITY:
            return self._quality(prepared, preferences.quality)
        target = preferences.target_size_kib * 1024
        lower, upper = 1, 95
        smallest: bytes | None = None
        best: bytes | None = None
        while lower <= upper:
            quality = (lower + upper) // 2
            encoded = self._quality(prepared, quality)
            if smallest is None or len(encoded) < len(smallest):
                smallest = encoded
            if len(encoded) <= target:
                best = encoded
                lower = quality + 1
            else:
                upper = quality - 1
        if best is not None:
            return best
        if smallest is None:
            raise RuntimeError("JPEG quality search produced no encoded candidate.")
        return smallest

    @staticmethod
    def _prepare(image: Image.Image) -> Image.Image:
        """Flatten transparency against white, matching Substitute companion appearance."""
        if image.mode in {"RGBA", "LA"} or (
            image.mode == "P" and "transparency" in image.info
        ):
            rgba = image.convert("RGBA")
            background = Image.new("RGBA", rgba.size, (255, 255, 255, 255))
            background.alpha_composite(rgba)
            return background.convert("RGB")
        return image.convert("RGB")

    @staticmethod
    def _quality(image: Image.Image, quality: int) -> bytes:
        """Encode optimized pixels without forwarding source image metadata."""
        buffer = BytesIO()
        image.save(buffer, format="JPEG", quality=quality, optimize=True)
        return buffer.getvalue()
