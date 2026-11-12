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
"""Describe public GIMP operations for preparing and committing one image layer."""

from __future__ import annotations

from typing import Protocol

from .file_api import NativeFile
from .image_api import ImageLookup, NativeImage, NativeLayer


class NativeOutputLayer(NativeLayer, Protocol):
    """Prepare native layer properties while the layer remains unattached."""

    def get_width(self) -> int:
        """Read decoded intrinsic pixel width."""
        ...

    def get_height(self) -> int:
        """Read decoded intrinsic pixel height."""
        ...

    def set_name(self, name: str) -> bool:
        """Set literal native layer text before insertion."""
        ...

    def set_offsets(self, left: int, top: int) -> bool:
        """Apply the shared placement owner's exact pixel origin."""
        ...


class NativeOutputImage(NativeImage, Protocol):
    """Commit one prepared layer through the public native image API."""

    def insert_layer(
        self, layer: NativeOutputLayer, parent: NativeLayer | None, position: int
    ) -> bool:
        """Insert one fully prepared layer as one native undoable mutation."""
        ...


class NativeRunModes(Protocol):
    """Select the public noninteractive native decoder mode."""

    NONINTERACTIVE: int


class NativeImageTypes(Protocol):
    """Identify the supported native RGB destination type."""

    RGB: int


class GimpOutputApi(Protocol):
    """Separate writable native operations from read-only capture access."""

    Image: ImageLookup[NativeOutputImage]
    RunMode: NativeRunModes
    ImageBaseType: NativeImageTypes

    def file_load_layer(
        self, run_mode: int, image: NativeOutputImage, file: NativeFile
    ) -> NativeOutputLayer | None:
        """Decode a layer without inserting it into the destination stack."""
        ...
