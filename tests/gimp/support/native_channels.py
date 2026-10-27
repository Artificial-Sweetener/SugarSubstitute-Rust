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
"""Supply typed channel SDK boundaries for native ownership and persisted coverage proofs."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from dataclasses import dataclass, field

from integrations.gimp.native.channel_api import (
    NativeChannel,
    NativeChannelImage,
    NativeChannelFactory,
    NativeParasiteFactory,
)
from integrations.gimp.native.image_api import (
    ImageLookup,
    NativeLayer,
    NativeParasite,
    NativePrecisions,
)
from .native_images import FakeBuffer, FakeImage, FakeLayer, FakePrecisions


@dataclass
class FakeParasite:
    """Retain exact GI metadata bytes."""

    name: str
    data: bytes

    def get_data(self) -> Sequence[int]:
        """Expose native byte values without serializing mask pixels."""
        return self.data


@dataclass
class FakeChannel(FakeLayer):
    """Add only the external channel operations used by production ownership."""

    markers: dict[str, NativeParasite] = field(default_factory=dict)
    fail_marker: bool = False

    def get_buffer(self) -> FakeBuffer:
        """Borrow the fixture's writable GEGL boundary."""
        return self.buffer

    def update(self, x: int, y: int, width: int, height: int) -> bool:
        """Accept the exact native invalidation extent."""
        return True

    def attach_parasite(self, parasite: NativeParasite) -> bool:
        """Store native metadata without assigning application semantics."""
        if not isinstance(parasite, FakeParasite):
            raise TypeError("Expected the typed native metadata fixture.")
        self.markers[parasite.name] = parasite
        return not self.fail_marker

    def get_parasite(self, name: str) -> NativeParasite | None:
        """Resolve opaque native metadata independently of the channel label."""
        return self.markers.get(name)


@dataclass
class FakeChannelImage(FakeImage):
    """Model the SDK's mutually exclusive layer and channel editing surfaces."""

    selected_channels: list[NativeChannel] = field(default_factory=list)
    inserted_channels: list[NativeChannel] = field(default_factory=list)
    fail_channel_insert: bool = False
    fail_layer_select: bool = False
    on_channel_insert: Callable[[], None] | None = None

    def get_channel_by_tattoo(self, tattoo: int) -> NativeChannel | None:
        """Narrow the writable SDK fixture at its own external boundary."""
        channel = self.channels.get(tattoo)
        if channel is not None and not isinstance(channel, FakeChannel):
            raise TypeError("Expected the typed native channel fixture.")
        return channel

    def insert_channel(
        self, channel: NativeChannel, parent: NativeChannel | None, position: int
    ) -> bool:
        """Record the production owner's single native mutation boundary."""
        if self.fail_channel_insert:
            return False
        self.channels[channel.get_tattoo()] = channel
        self.inserted_channels.append(channel)
        self.selected_channels = [channel]
        if self.on_channel_insert:
            self.on_channel_insert()
        return True

    def get_selected_channels(self) -> Sequence[NativeChannel]:
        """Read explicit native editing focus."""
        return self.selected_channels

    def set_selected_channels(self, channels: Sequence[NativeChannel]) -> bool:
        """Match the public PDB setter, which prepends selected channel identities."""
        self.selected_channels = list(reversed(channels))
        return True

    def set_selected_layers(self, layers: Sequence[NativeLayer]) -> bool:
        """Match the PDB layer-list order while retiring the previous channel surface."""
        if self.fail_layer_select:
            return False
        values: list[FakeLayer] = []
        for layer in layers:
            if not isinstance(layer, FakeLayer):
                raise TypeError("Expected the typed native layer fixture.")
            values.append(layer)
        self.selected = list(reversed(values))
        self.selected_channels = []
        return True


@dataclass
class FakeChannelLookup:
    """Resolve only the explicit native image used by this fixture."""

    image: FakeChannelImage

    def get_by_id(self, image_id: int) -> NativeChannelImage | None:
        """Never substitute another native image."""
        return self.image if self.image.image_id == image_id else None


@dataclass
class FakeChannelFactory:
    """Retain prepared SDK resources for success and failure assertions."""

    created: list[FakeChannel] = field(default_factory=list)
    fail_marker: bool = False
    on_create: Callable[[], None] | None = None

    def new(
        self,
        image: NativeChannelImage,
        name: str,
        width: int,
        height: int,
        opacity: float,
        color: object,
    ) -> NativeChannel:
        """Return one unattached native channel with a preallocated persistent identity."""
        channel = FakeChannel(
            FakeBuffer(bytes(width * height)),
            tattoo=100 + len(self.created),
            name=name,
            fail_marker=self.fail_marker,
        )
        self.created.append(channel)
        if self.on_create:
            self.on_create()
        return channel


class FakeParasiteFactory:
    """Construct opaque native metadata records."""

    def new(self, name: str, flags: int, data: bytes) -> NativeParasite:
        """Retain the admitted native persistence bytes."""
        return FakeParasite(name, data)


@dataclass
class FakeChannelApi:
    """Inject only the external GI services used by native authored masks."""

    Image: ImageLookup[NativeChannelImage]
    Channel: NativeChannelFactory = field(default_factory=FakeChannelFactory)
    Parasite: NativeParasiteFactory = field(default_factory=FakeParasiteFactory)
    PARASITE_PERSISTENT: int = 1
    Precision: NativePrecisions = field(default_factory=FakePrecisions)

    def displays_flush(self) -> bool:
        """Accept native redraw publication independently of desktop rendering."""
        return True
