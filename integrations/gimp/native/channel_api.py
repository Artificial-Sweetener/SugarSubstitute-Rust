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
"""Describe public native channel mutation separately from read-only image capture."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol

from .image_api import (
    ImageLookup,
    NativeBuffer,
    NativeChannelSource,
    NativeImage,
    NativeLayer,
    NativeParasite,
    NativePrecisions,
)


class NativeWritableBuffer(NativeBuffer, Protocol):
    """Write admitted coverage into an unattached native channel."""

    def set(self, rectangle: object, format_name: str, data: bytes) -> None:
        """Convert exact coverage bytes through GEGL's native pixel boundary."""
        ...

    def flush(self) -> None:
        """Commit prepared buffer writes before native channel insertion."""
        ...


class NativeChannel(NativeChannelSource, Protocol):
    """Prepare coverage and restore native editing surfaces without altering image layers."""

    def get_buffer(self) -> NativeWritableBuffer:
        """Borrow writable native coverage."""
        ...

    def get_tattoo(self) -> int:
        """Read the persistent channel identity."""
        ...

    def get_visible(self) -> bool:
        """Read the channel overlay state."""
        ...

    def set_visible(self, visible: bool) -> bool:
        """Expose or retire the native editing overlay."""
        ...

    def is_valid(self) -> bool:
        """Reject native resources removed through undo or document closure."""
        ...

    def delete(self) -> bool:
        """Release an uncommitted channel after preparation fails."""
        ...

    def update(self, x: int, y: int, width: int, height: int) -> bool:
        """Publish the prepared native coverage extent."""
        ...

    def attach_parasite(self, parasite: NativeParasite) -> bool:
        """Keep the authored coverage marker inside the XCF."""
        ...


class NativeChannelImage(NativeImage, Protocol):
    """Own channel insertion and native editing selection for one explicit image."""

    def get_channel_by_tattoo(self, tattoo: int) -> NativeChannel | None:
        """Resolve an editable channel inside the bound image."""
        ...

    def insert_channel(
        self, channel: NativeChannel, parent: NativeChannel | None, position: int
    ) -> bool:
        """Commit one fully prepared mask channel as one native undo step."""
        ...

    def get_selected_channels(self) -> Sequence[NativeChannel]:
        """Read native channel focus without inferring it from visibility."""
        ...

    def set_selected_channels(self, channels: Sequence[NativeChannel]) -> bool:
        """Let GIMP's normal tools edit the selected authored coverage."""
        ...

    def set_selected_layers(self, layers: Sequence[NativeLayer]) -> bool:
        """Restore the native layer surface after mask editing ends."""
        ...


class NativeChannelFactory(Protocol):
    """Prepare a channel without attaching it to a user's image."""

    def new(
        self,
        image: NativeChannelImage,
        name: str,
        width: int,
        height: int,
        opacity: float,
        color: object,
    ) -> NativeChannel | None:
        """Create full-canvas native coverage with GIMP's standard overlay."""
        ...


class NativeParasiteFactory(Protocol):
    """Create a versioned native persistence marker."""

    def new(self, name: str, flags: int, data: bytes) -> NativeParasite:
        """Store exact bounded bytes in the host's metadata representation."""
        ...


class GimpChannelApi(Protocol):
    """Isolate dynamic native channel symbols from the viewer and application services."""

    Image: ImageLookup[NativeChannelImage]
    Channel: NativeChannelFactory
    Parasite: NativeParasiteFactory
    PARASITE_PERSISTENT: int
    Precision: NativePrecisions

    def displays_flush(self) -> bool:
        """Publish native editing surface changes to GIMP displays."""
        ...
