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
"""Preserve native editing identities and their exposed order around authored mask operations."""

from __future__ import annotations

from typing import Literal

import pytest

from integrations.gimp.native.channel_editing import NativeChannelEditing
from integrations.gimp.native.channels import NativeChannels
from integrations.gimp.native.images import describe_image
from integrations.gimp.native.layers import primary_layer
from tests.gimp.support.native_channels import (
    FakeChannel,
    FakeChannelApi,
    FakeChannelFactory,
    FakeChannelImage,
    FakeChannelLookup,
)
from tests.gimp.support.native_images import FakeBuffer, FakeGeglApi, FakeLayer

FocusKind = Literal["layers", "channels"]


def fixture(
    kind: FocusKind,
) -> tuple[FakeChannelImage, NativeChannels, NativeChannelEditing, list[int]]:
    """Retain a deliberately non-stack-ordered native focus selection at the SDK boundary."""
    layers = [FakeLayer(FakeBuffer(b""), tattoo=index) for index in (1, 2, 3)]
    originals = [
        FakeChannel(FakeBuffer(bytes(48)), tattoo=index) for index in (21, 22, 23)
    ]
    image = FakeChannelImage(
        layers=layers, channels={item.tattoo: item for item in originals}
    )
    if kind == "layers":
        image.selected = [layers[1], layers[0], layers[2]]
    else:
        image.selected_channels = [originals[1], originals[0], originals[2]]
    expected = [2, 1, 3] if kind == "layers" else [22, 21, 23]
    api = FakeChannelApi(FakeChannelLookup(image), FakeChannelFactory())

    def report(error: Exception) -> None:
        """Reject an unexpected native cleanup or redraw failure."""
        raise error

    return (
        image,
        NativeChannels(api, FakeGeglApi(), 192, report),
        NativeChannelEditing(api, report),
        expected,
    )


def selected(image: FakeChannelImage, kind: FocusKind) -> list[int]:
    """Read the exact ordered native identities exposed to the adapter's context owner."""
    items = (
        image.get_selected_layers()
        if kind == "layers"
        else image.get_selected_channels()
    )
    return [item.get_tattoo() for item in items]


def set_observed_focus(
    image: FakeChannelImage, kind: FocusKind, expected: list[int]
) -> None:
    """Initialize editing's captured focus independently of the preceding mask insertion."""
    if kind == "layers":
        image.selected = sorted(
            image.selected, key=lambda item: expected.index(item.get_tattoo())
        )
    else:
        image.selected_channels = sorted(
            image.selected_channels, key=lambda item: expected.index(item.get_tattoo())
        )


@pytest.mark.parametrize("kind", ["layers", "channels"])
def test_mask_creation_preserves_the_existing_native_focus_order(
    kind: FocusKind,
) -> None:
    """Restore the original current layer or channels after the committed mask insertion."""
    image, channels, editing, expected = fixture(kind)
    try:
        tattoo = channels.create(
            describe_image(image), "Ordered mask", "blank", None, lambda: False
        )
        assert tattoo in image.channels
        assert selected(image, kind) == expected
        if kind == "layers":
            assert primary_layer(image) == expected[0]
    finally:
        channels.close()
        editing.close()


@pytest.mark.parametrize("kind", ["layers", "channels"])
@pytest.mark.parametrize("close", [False, True])
def test_mask_editing_release_preserves_the_original_native_focus_order(
    kind: FocusKind, close: bool
) -> None:
    """Restore the same current native item after explicit release or viewer shutdown."""
    image, channels, editing, expected = fixture(kind)
    try:
        tattoo = channels.create(
            describe_image(image), "Ordered mask", "blank", None, lambda: False
        )
        set_observed_focus(image, kind, expected)
        assert selected(image, kind) == expected
        editing.select(describe_image(image), tattoo)
        if close:
            editing.close()
        else:
            editing.release(tattoo)
        assert selected(image, kind) == expected
        assert not image.channels[tattoo].get_visible()
        if kind == "layers":
            assert primary_layer(image) == expected[0]
    finally:
        channels.close()
        editing.close()


@pytest.mark.parametrize("kind", ["layers", "channels"])
def test_focus_restoration_keeps_surviving_native_items_in_their_captured_order(
    kind: FocusKind,
) -> None:
    """Exclude a native item deleted during editing without promoting an unrelated item."""
    image, channels, editing, expected = fixture(kind)
    try:
        tattoo = channels.create(
            describe_image(image), "Ordered mask", "blank", None, lambda: False
        )
        set_observed_focus(image, kind, expected)
        assert selected(image, kind) == expected
        editing.select(describe_image(image), tattoo)
        if kind == "layers":
            image.layers = [
                layer for layer in image.layers if layer.get_tattoo() != expected[1]
            ]
        else:
            del image.channels[expected[1]]
        editing.release(tattoo)
        assert selected(image, kind) == [expected[0], expected[2]]
    finally:
        channels.close()
        editing.close()
