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
"""Bind source identities to GIMP's renumbered frozen items without modifying the user's document."""

from __future__ import annotations

from dataclasses import dataclass

from .channel_identity import is_authored_channel
from .image_api import NativeImage
from .layers import layer_catalog, primary_layer
from .model import NativeLayerInfo, StaleNativeImage


@dataclass(frozen=True, slots=True)
class NativeChannelIdentity:
    """Qualify one channel's ordered correspondence across a native image clone."""

    tattoo: int
    name: str
    authored: bool


@dataclass(frozen=True, slots=True)
class NativeItemCatalog:
    """Detect topology and selection changes around the host's atomic image duplication."""

    layers: tuple[NativeLayerInfo, ...]
    primary: int | None
    channels: tuple[NativeChannelIdentity, ...]


@dataclass(frozen=True, slots=True)
class NativeSnapshotBindings:
    """Translate original native identities only inside this owned offscreen image."""

    layers: dict[int, int]
    channels: dict[int, int]


def native_item_catalog(image: NativeImage) -> NativeItemCatalog:
    """Bound metadata reads without treating names or pixel state as native identity."""
    channels = image.get_channels()
    if len(channels) > 4096:
        raise RuntimeError("The GIMP channel catalog exceeds its supported size.")
    values = tuple(
        NativeChannelIdentity(
            channel.get_tattoo(), channel.get_name(), is_authored_channel(channel)
        )
        for channel in channels
    )
    if len({value.tattoo for value in values}) != len(values):
        raise RuntimeError("GIMP returned duplicate channel identities.")
    return NativeItemCatalog(layer_catalog(image), primary_layer(image), values)


def bind_snapshot_items(
    before: NativeItemCatalog, after: NativeItemCatalog, frozen: NativeItemCatalog
) -> NativeSnapshotBindings:
    """Use the clone's native order while refusing topology changes or an inconsistent copy.

    GIMP's gimp_item_set_image assigns new tattoos when gimp_image_duplicate
    converts layers and channels into another image. The host preserves stack
    order and selected paths, so correspondence is qualified before and after
    that operation rather than guessed from a copied tattoo or editable name.
    """
    if (
        before != after
        or len(before.layers) != len(frozen.layers)
        or len(before.channels) != len(frozen.channels)
    ):
        raise StaleNativeImage(
            "The GIMP item catalog changed while its contents were frozen."
        )
    layer_map = {
        original.tattoo: copy.tattoo
        for original, copy in zip(before.layers, frozen.layers)
    }
    reverse = {copy: original for original, copy in layer_map.items()}
    for original, copy in zip(before.layers, frozen.layers):
        if (original.name, original.depth, original.visible, original.has_mask) != (
            copy.name,
            copy.depth,
            copy.visible,
            copy.has_mask,
        ) or original.parent_tattoo != (
            None if copy.parent_tattoo is None else reverse.get(copy.parent_tattoo)
        ):
            raise StaleNativeImage(
                "The GIMP layer copy differs from its admitted source hierarchy."
            )
    if before.primary != (
        None if frozen.primary is None else reverse.get(frozen.primary)
    ):
        raise StaleNativeImage(
            "The selected GIMP layer changed while its contents were frozen."
        )
    channel_map: dict[int, int] = {}
    for original_channel, copy_channel in zip(before.channels, frozen.channels):
        if (original_channel.name, original_channel.authored) != (
            copy_channel.name,
            copy_channel.authored,
        ):
            raise StaleNativeImage(
                "The GIMP channel copy differs from its admitted source stack."
            )
        channel_map[original_channel.tattoo] = copy_channel.tattoo
    return NativeSnapshotBindings(layer_map, channel_map)
