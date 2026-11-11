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
"""Supply typed native file and insertion boundaries for output transaction proofs."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

from integrations.gimp.native.file_api import NativeFile, NativeFileFactory
from integrations.gimp.native.image_api import ImageLookup, NativeLayer
from integrations.gimp.native.output_api import (
    NativeImageTypes,
    NativeOutputImage,
    NativeOutputLayer,
    NativeRunModes,
)
from .native_images import FakeBuffer, FakeLayer
from .native_channels import FakeChannelImage


@dataclass
class FakeFile:
    """Expose the owned file produced by the real temporary-file owner."""

    path: str

    def get_path(self) -> str | None:
        """Read the fixture's local path."""
        return self.path


class FakeFileFactory:
    """Wrap actual fixture files without loading Gio."""

    def new_for_path(self, path: str) -> NativeFile:
        """Create a typed file boundary."""
        return FakeFile(path)


@dataclass
class FakeGioApi:
    """Inject the installed file factory's narrow surface."""

    File: NativeFileFactory = field(default_factory=FakeFileFactory)


@dataclass
class FakeOutputLayer(FakeLayer):
    """Record preparation separately from target-document insertion."""

    width: int = 3
    height: int = 2
    left: int = 0
    top: int = 0
    fail_prepare: bool = False

    def get_width(self) -> int:
        """Read native decoded width."""
        return self.width

    def get_height(self) -> int:
        """Read native decoded height."""
        return self.height

    def set_name(self, name: str) -> bool:
        """Prepare the literal label off-image."""
        self.name = name
        return not self.fail_prepare

    def set_offsets(self, left: int, top: int) -> bool:
        """Prepare exact shared coordinates off-image."""
        self.left, self.top = left, top
        return not self.fail_prepare


@dataclass
class FakeOutputImage(FakeChannelImage):
    """Observe one native mutation and an optional concurrent cancellation."""

    inserted: list[NativeOutputLayer] = field(default_factory=list)
    fail_insert: bool = False
    on_insert: Callable[[], None] | None = None

    def insert_layer(
        self, layer: NativeOutputLayer, parent: NativeLayer | None, position: int
    ) -> bool:
        """Commit only after preparation has completed."""
        if parent is not None or position != 0:
            raise ValueError("Unexpected output insertion point.")
        if self.fail_insert:
            return False
        self.inserted.append(layer)
        self.dirty = True
        if self.on_insert is not None:
            self.on_insert()
        return True


@dataclass
class FakeOutputLookup:
    """Return explicit writable image identities."""

    image: FakeOutputImage

    def get_by_id(self, image_id: int) -> NativeOutputImage | None:
        """Keep invalid proxies available for stale-document tests."""
        return self.image if image_id == self.image.image_id else None


class FakeRunModes:
    """Expose noninteractive native decoding."""

    NONINTERACTIVE: int = 1


class FakeImageTypes:
    """Expose the supported native RGB model."""

    RGB: int = 0


@dataclass
class FakeOutputApi:
    """Record native decoding while borrowing real temporary PNG files."""

    Image: ImageLookup[NativeOutputImage]
    RunMode: NativeRunModes = field(default_factory=FakeRunModes)
    ImageBaseType: NativeImageTypes = field(default_factory=FakeImageTypes)
    decoded: list[FakeOutputLayer] = field(default_factory=list)
    loaded: list[bytes] = field(default_factory=list)
    width: int = 3
    height: int = 2
    fail_prepare: bool = False
    fail_delete: bool = False
    on_load: Callable[[], None] | None = None

    def file_load_layer(
        self, run_mode: int, image: NativeOutputImage, file: NativeFile
    ) -> NativeOutputLayer:
        """Verify native admission and permit deterministic failure during decoding."""
        if run_mode != self.RunMode.NONINTERACTIVE:
            raise ValueError("Interactive native decoding is not allowed.")
        path = file.get_path()
        if path is None:
            raise ValueError("Expected an owned local decoder input.")
        self.loaded.append(Path(path).read_bytes())
        if self.on_load is not None:
            self.on_load()
        layer = FakeOutputLayer(
            FakeBuffer(b""),
            tattoo=123,
            width=self.width,
            height=self.height,
            fail_prepare=self.fail_prepare,
            fail_delete=self.fail_delete,
        )
        self.decoded.append(layer)
        return layer
