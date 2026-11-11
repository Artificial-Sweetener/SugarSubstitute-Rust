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
"""Model only GIMP's external selection-bounds and canvas-resize calls."""

from __future__ import annotations

from dataclasses import dataclass, field

from integrations.gimp.native.image_api import NativeImage
from .native_images import FakeImage, FakeImageLookup


@dataclass
class FakeGeometryImages:
    """Record native bounds mutations independently of adapter calculations."""

    lookup: FakeImageLookup
    calls: list[tuple[int, int, int, int]] = field(default_factory=list)
    fail: bool = False

    def get_by_id(self, image_id: int) -> NativeImage | None:
        """Resolve the same explicit identity used by source capture."""
        return self.lookup.get_by_id(image_id)

    def resize(
        self, image: NativeImage, width: int, height: int, left: int, top: int
    ) -> bool:
        """Apply SDK dimensions without replacing the target image or its selected layers."""
        self.calls.append((width, height, left, top))
        if self.fail:
            return False
        if not isinstance(image, FakeImage):
            raise TypeError("The geometry fixture requires its native image boundary.")
        image.width, image.height = width, height
        return True


@dataclass
class FakeGeometrySelection:
    """Expose configured native selection extents without deriving them in product code."""

    bounds_value: tuple[bool, bool, int, int, int, int] = (True, False, 0, 0, 0, 0)

    def bounds(self, image: NativeImage) -> tuple[bool, bool, int, int, int, int]:
        """Return the exact public SDK tuple."""
        return self.bounds_value


@dataclass
class FakeGeometryApi:
    """Compose the installed namespace's geometry surfaces."""

    Image: FakeGeometryImages
    Selection: FakeGeometrySelection = field(default_factory=FakeGeometrySelection)
