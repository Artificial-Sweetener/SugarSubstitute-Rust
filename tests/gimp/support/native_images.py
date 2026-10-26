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
"""Supply typed native SDK boundaries for deterministic image lifetime proofs."""

from __future__ import annotations

from collections.abc import Callable, Sequence
from copy import deepcopy
from dataclasses import dataclass, field

from integrations.gimp.native.image_api import (
    ImageLookup,
    LayerFactory,
    NativeBuffer,
    NativeChannelSource,
    NativeDrawable,
    NativeImage,
    NativeLayer,
    NativePrecisions,
    NativeBaseTypes,
    NativeComponentTypes,
)
from integrations.gimp.native.sdk import AbyssModes, ColorFactory, RectangleFactory


@dataclass
class FakeBuffer:
    """Record native pixel requests while returning explicitly configured bytes."""

    data: bytes
    calls: list[tuple[object, float, str, int]] = field(default_factory=list)

    def get(
        self, rectangle: object, scale: float, format_name: str, repeat_mode: int
    ) -> bytes:
        """Record only the external SDK call being exercised."""
        self.calls.append((rectangle, scale, format_name, repeat_mode))
        return self.data

    def set(self, rectangle: object, format_name: str, data: bytes) -> None:
        """Receive exact bytes at the native write boundary."""
        self.data = data

    def flush(self) -> None:
        """Keep in-memory writes immediately visible to native fixture readers."""


@dataclass
class FakeDrawable:
    """Expose a deterministic external pixel buffer."""

    buffer: FakeBuffer

    def get_buffer(self) -> NativeBuffer:
        """Borrow the fixture's pixel boundary."""
        return self.buffer


@dataclass
class FakeLayer(FakeDrawable):
    """Retain visibility and child identity like a native SDK layer."""

    tattoo: int = 1
    name: str = "Layer"
    visible: bool = True
    children: list[FakeLayer] = field(default_factory=list)
    mask: FakeDrawable | None = None
    deleted: bool = False
    fail_delete: bool = False
    delete_calls: int = 0

    def get_tattoo(self) -> int:
        """Read stable native identity."""
        return self.tattoo

    def is_valid(self) -> bool:
        """Retain fixture proxies after deletion like the native SDK."""
        return not self.deleted

    def get_name(self) -> str:
        """Read the fixture label."""
        return self.name

    def is_group(self) -> bool:
        """Identify fixture groups."""
        return bool(self.children)

    def get_children(self) -> Sequence[NativeLayer] | None:
        """Read the ordered child stack."""
        return self.children

    def get_visible(self) -> bool:
        """Read native visibility."""
        return self.visible

    def set_visible(self, visible: bool) -> bool:
        """Change only this fixture layer."""
        self.visible = visible
        return True

    def get_mask(self) -> NativeDrawable | None:
        """Read the fixture mask."""
        return self.mask

    def delete(self) -> bool:
        """Record release of an unattached native layer."""
        self.delete_calls += 1
        if self.fail_delete:
            return False
        self.deleted = True
        return True


@dataclass
class FakeProfile:
    """Expose deterministic ICC bytes."""

    data: bytes

    def get_icc_profile(self) -> bytes:
        """Read exact native profile bytes."""
        return self.data


@dataclass
class FakeImage:
    """Model an explicit image and its independently owned duplicates."""

    image_id: int = 1
    width: int = 8
    height: int = 6
    name: str = "Native image"
    valid: bool = True
    dirty: bool = False
    profile: bytes = b"native ICC profile"
    precision: int = 0
    base_type: int = 0
    layers: list[FakeLayer] = field(default_factory=list)
    selected: list[FakeLayer] = field(default_factory=list)
    channels: dict[int, NativeChannelSource] = field(default_factory=dict)
    components: dict[int, bool] = field(default_factory=dict)
    selection: FakeDrawable = field(
        default_factory=lambda: FakeDrawable(FakeBuffer(bytes([127]) * 48))
    )
    projection: bytes = bytes([10, 200, 30, 255]) * 48
    copies: list[FakeImage] = field(default_factory=list)
    on_duplicate: Callable[[FakeImage], None] | None = None
    delete_calls: int = 0
    fail_delete: bool = False
    width_reads: int = 0
    change_width_at: int | None = None

    def get_id(self) -> int:
        """Read process-local identity."""
        return self.image_id

    def is_valid(self) -> bool:
        """Retain deleted proxies to exercise native validity checks."""
        return self.valid

    def is_dirty(self) -> bool:
        """Read native mutation state independently of geometry."""
        return self.dirty

    def get_name(self) -> str:
        """Read the fixture label."""
        return self.name

    def get_width(self) -> int:
        """Permit a deterministic resize during metadata capture."""
        self.width_reads += 1
        if self.change_width_at == self.width_reads:
            self.width += 1
        return self.width

    def get_height(self) -> int:
        """Read the fixture height."""
        return self.height

    def get_base_type(self) -> int:
        """Read native color model."""
        return self.base_type

    def get_precision(self) -> int:
        """Read native precision."""
        return self.precision

    def get_effective_color_profile(self) -> FakeProfile:
        """Read native color interpretation."""
        return FakeProfile(self.profile)

    def get_layers(self) -> Sequence[NativeLayer]:
        """Read native top-level layers."""
        return self.layers

    def get_selected_layers(self) -> Sequence[NativeLayer]:
        """Read native selected layers."""
        return self.selected

    def get_layer_by_tattoo(self, tattoo: int) -> NativeLayer | None:
        """Resolve identities only within this image."""
        pending = list(self.layers)
        while pending:
            layer = pending.pop()
            if layer.tattoo == tattoo:
                return layer
            pending.extend(layer.children)
        return None

    def get_selection(self) -> NativeDrawable:
        """Read native selection coverage."""
        return self.selection

    def get_channel_by_tattoo(self, tattoo: int) -> NativeChannelSource | None:
        """Resolve coverage inside this exact frozen fixture image."""
        return self.channels.get(tattoo)

    def get_channels(self) -> Sequence[NativeChannelSource]:
        """Read channel order independently of names and native identity values."""
        return list(self.channels.values())

    def set_component_visible(self, component: int, visible: bool) -> bool:
        """Change only the external display-component state of this fixture image."""
        self.components[component] = visible
        return True

    def duplicate(self) -> NativeImage:
        """Freeze values without retaining mutable source-layer objects."""
        layers, selected = deepcopy((self.layers, self.selected))
        duplicate = FakeImage(
            image_id=self.image_id + 1000 + len(self.copies),
            width=self.width,
            height=self.height,
            name=self.name,
            profile=self.profile,
            precision=self.precision,
            base_type=self.base_type,
            layers=layers,
            selected=selected,
            channels=deepcopy(self.channels),
            components=dict(self.components),
            selection=deepcopy(self.selection),
            projection=self.projection,
        )
        self.copies.append(duplicate)
        if self.on_duplicate is not None:
            self.on_duplicate(duplicate)
        return duplicate

    def delete(self) -> bool:
        """Record native cleanup while permitting failure-path coverage."""
        self.delete_calls += 1
        if self.fail_delete:
            return False
        self.valid = False
        return True


@dataclass
class FakeImageLookup:
    """Retain invalid proxies as the real SDK can."""

    images: dict[int, FakeImage]

    def get_by_id(self, image_id: int) -> NativeImage | None:
        """Resolve an explicit process-local identifier."""
        return self.images.get(image_id)


@dataclass
class FakeProjectionFactory:
    """Record projected native resources and support a decoder failure."""

    created: list[FakeLayer] = field(default_factory=list)
    fail: bool = False
    fail_delete: bool = False
    visibility_reads: list[list[tuple[int, bool]]] = field(default_factory=list)

    def new_from_visible(
        self, image: NativeImage, dest_image: NativeImage, name: str
    ) -> NativeLayer:
        """Return fixture pixels from the frozen image supplied by the production owner."""
        if self.fail:
            raise RuntimeError("Native projection failed.")
        if not isinstance(image, FakeImage) or image is not dest_image:
            raise TypeError("Unexpected native projection target.")
        self.visibility_reads.append(
            [(layer.get_tattoo(), layer.get_visible()) for layer in image.layers]
        )
        layer = FakeLayer(
            FakeBuffer(image.projection), name=name, fail_delete=self.fail_delete
        )
        self.created.append(layer)
        return layer


class FakePrecisions:
    """Expose only the native format discriminator used by coverage translation."""

    U8_NON_LINEAR: int = 1


class FakeBaseTypes:
    """Expose the SDK's color-model selectors."""

    RGB: int = 0
    GRAY: int = 1
    INDEXED: int = 2


class FakeComponentTypes:
    """Expose distinct native component selectors for projection proofs."""

    RED: int = 0
    GREEN: int = 1
    BLUE: int = 2
    ALPHA: int = 3
    GRAY: int = 4
    INDEXED: int = 5


@dataclass
class FakeGimpApi:
    """Inject exact typed image and projection boundaries."""

    Image: ImageLookup[NativeImage]
    Layer: LayerFactory
    Precision: NativePrecisions = field(default_factory=FakePrecisions)
    ImageBaseType: NativeBaseTypes = field(default_factory=FakeBaseTypes)
    ChannelType: NativeComponentTypes = field(default_factory=FakeComponentTypes)

    def get_images(self) -> Sequence[NativeImage]:
        """Require an explicit inventory fixture for tests that exercise global image listing."""
        raise NotImplementedError(
            "This fixture does not provide a global native image inventory."
        )


class FakeRectangles:
    """Represent GEGL regions without loading its native runtime."""

    def new(self, x: int, y: int, width: int, height: int) -> object:
        """Retain the exact native region arguments."""
        return (x, y, width, height)


class FakeAbyss:
    """Expose the native clamp mode."""

    CLAMP: int = 1


class FakeColors:
    """Retain literal native overlay colors without introducing color policy."""

    def new(self, value: str) -> object:
        """Return the external color descriptor supplied by the adapter."""
        return value


@dataclass
class FakeGeglApi:
    """Inject typed native rectangle construction and sampling constants."""

    Rectangle: RectangleFactory = field(default_factory=FakeRectangles)
    AbyssPolicy: AbyssModes = field(default_factory=FakeAbyss)
    Color: ColorFactory = field(default_factory=FakeColors)

    def init(self, argv: list[str] | None) -> list[str] | None:
        """Keep fixture initialization independent of native process state."""
        return argv
