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
"""Describe the public libgimp image surface used by native translation."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol, TypeVar


class NativeBuffer(Protocol):
    """Read GEGL pixels without importing its dynamic module into adapter logic."""

    def get(
        self, rectangle: object, scale: float, format_name: str, repeat_mode: int
    ) -> bytes:
        """Read a scaled region in the requested native pixel format."""
        ...


class NativeDrawable(Protocol):
    """Expose the pixel buffer of one native drawable."""

    def get_buffer(self) -> NativeBuffer:
        """Borrow the drawable's GEGL pixel access."""
        ...


class NativeParasite(Protocol):
    """Read persistent native metadata without interpreting application data."""

    def get_data(self) -> Sequence[int]:
        """Read the exact byte values supplied by the GI boundary."""
        ...


class NativeChannelSource(NativeDrawable, Protocol):
    """Read one saved coverage channel from a coherent native image."""

    def get_parasite(self, name: str) -> NativeParasite | None:
        """Read the bounded authored-channel identity marker."""
        ...

    def get_tattoo(self) -> int:
        """Read identity within the channel's own image."""
        ...

    def get_name(self) -> str:
        """Qualify clone correspondence without using editable names as persistent identities."""
        ...

    def get_visible(self) -> bool:
        """Read native display overlay visibility."""
        ...

    def set_visible(self, visible: bool) -> bool:
        """Suppress a display overlay only inside an owned offscreen image."""
        ...


class NativeLayer(NativeDrawable, Protocol):
    """Expose layer identity and visibility on a frozen native image."""

    def get_tattoo(self) -> int:
        """Read the identity retained when an image is duplicated or saved."""
        ...

    def is_valid(self) -> bool:
        """Distinguish retained proxies from native layer resources."""
        ...

    def get_name(self) -> str:
        """Read the native label without interpreting its contents."""
        ...

    def is_group(self) -> bool:
        """Identify layers with a child stack."""
        ...

    def get_children(self) -> Sequence[NativeLayer] | None:
        """Read group children in native top-to-bottom order."""
        ...

    def get_visible(self) -> bool:
        """Read the layer's own visibility flag."""
        ...

    def set_visible(self, visible: bool) -> bool:
        """Change visibility on an adapter-owned snapshot."""
        ...

    def get_mask(self) -> NativeDrawable | None:
        """Read the optional native layer mask."""
        ...

    def delete(self) -> bool:
        """Release an unattached projection layer after its pixels are read."""
        ...


class NativeProfile(Protocol):
    """Expose effective ICC data for an opaque native geometry identity."""

    def get_icc_profile(self) -> bytes:
        """Read the native profile's exact ICC bytes."""
        ...


class NativeImage(Protocol):
    """Describe image operations supported by the installed public GIMP API."""

    def get_id(self) -> int:
        """Read this GIMP process's image identity."""
        ...

    def is_valid(self) -> bool:
        """Distinguish a retained Python proxy from an open native image."""
        ...

    def is_dirty(self) -> bool:
        """Read document modification state without treating it as a content revision."""
        ...

    def get_name(self) -> str:
        """Read the native document label."""
        ...

    def get_width(self) -> int:
        """Read the canvas width in pixels."""
        ...

    def get_height(self) -> int:
        """Read the canvas height in pixels."""
        ...

    def get_base_type(self) -> int:
        """Read the native image color model."""
        ...

    def get_precision(self) -> int:
        """Read native channel precision for geometry qualification."""
        ...

    def get_effective_color_profile(self) -> NativeProfile:
        """Read the color interpretation used by the native image."""
        ...

    def get_layers(self) -> Sequence[NativeLayer]:
        """Read the top-level stack in top-to-bottom order."""
        ...

    def get_selected_layers(self) -> Sequence[NativeLayer]:
        """Read selected layers without substituting another image's selection."""
        ...

    def get_layer_by_tattoo(self, tattoo: int) -> NativeLayer | None:
        """Resolve a layer identity inside this exact image."""
        ...

    def get_selection(self) -> NativeDrawable:
        """Read full-canvas selection coverage."""
        ...

    def get_channel_by_tattoo(self, tattoo: int) -> NativeChannelSource | None:
        """Resolve saved coverage inside this exact native image."""
        ...

    def get_channels(self) -> Sequence[NativeChannelSource]:
        """Read the native channel order used by the host's image duplication operation."""
        ...

    def set_component_visible(self, component: int, visible: bool) -> bool:
        """Normalize display-only component filtering inside an owned snapshot."""
        ...

    def convert_rgb(self) -> bool:
        """Normalize an owned grayscale or indexed image for canonical color capture."""
        ...

    def duplicate(self) -> NativeImage:
        """Freeze native image contents for a coherent capture batch."""
        ...

    def delete(self) -> bool:
        """Release an adapter-owned image with no display."""
        ...


ImageType = TypeVar("ImageType", bound=NativeImage, covariant=True)


class ImageLookup(Protocol[ImageType]):
    """Resolve explicit native image identities."""

    def get_by_id(self, image_id: int) -> ImageType | None:
        """Read a native proxy that must also pass is_valid()."""
        ...


class LayerFactory(Protocol):
    """Render a native projection without attaching another user-visible layer."""

    def new_from_visible(
        self, image: NativeImage, dest_image: NativeImage, name: str
    ) -> NativeLayer | None:
        """Create an unattached layer from the current visible projection."""
        ...


class NativePrecisions(Protocol):
    """Identify GIMP's legacy channel format exception without hardcoding its enum value."""

    U8_NON_LINEAR: int


class NativeBaseTypes(Protocol):
    """Identify the native color model when suppressing display-only component filtering."""

    RGB: int
    GRAY: int
    INDEXED: int


class NativeComponentTypes(Protocol):
    """Name the public GIMP display-component selectors."""

    RED: int
    GREEN: int
    BLUE: int
    ALPHA: int


class GimpImageApi(Protocol):
    """Isolate the dynamic GI namespace behind its verified public surface."""

    Image: ImageLookup[NativeImage]
    Layer: LayerFactory
    Precision: NativePrecisions
    ImageBaseType: NativeBaseTypes
    ChannelType: NativeComponentTypes

    def get_images(self) -> Sequence[NativeImage]:
        """Inventory image lifetimes without treating list order as active-document state."""
        ...
