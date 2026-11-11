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
"""Prove authored-channel commit points, persistent identity, capture coherence and native focus ownership."""

from __future__ import annotations

import pytest

from integrations.gimp.native.channels import NativeChannels
from integrations.gimp.native.channel_editing import NativeChannelEditing
from integrations.gimp.native.channel_identity import MASK_MARKER, is_authored_channel
from integrations.gimp.native.images import describe_image
from integrations.gimp.native.model import NativeOperationCancelled, StaleNativeImage
from integrations.gimp.native.pixels import NativePixelReader
from integrations.gimp.native.snapshots import NativeSnapshots
from tests.gimp.support.native_channels import (
    FakeChannel,
    FakeChannelApi,
    FakeChannelFactory,
    FakeChannelImage,
    FakeChannelLookup,
)
from tests.gimp.support.native_images import (
    FakeBuffer,
    FakeGeglApi,
    FakeGimpApi,
    FakeImageLookup,
    FakeImage,
    FakeLayer,
    FakeProjectionFactory,
)


def fixture() -> (
    tuple[FakeChannelImage, FakeChannelFactory, NativeChannels, NativeChannelEditing]
):
    """Retain production ownership while replacing only public SDK boundaries."""
    layer = FakeLayer(FakeBuffer(b""), tattoo=1)
    image = FakeChannelImage(layers=[layer], selected=[layer])
    factory = FakeChannelFactory()
    api = FakeChannelApi(FakeChannelLookup(image), factory)

    def report(error: Exception) -> None:
        """Fail the proof when an unexpected native redraw diagnostic occurs."""
        raise error

    return (
        image,
        factory,
        NativeChannels(api, FakeGeglApi(), 192, report),
        NativeChannelEditing(api, report),
    )


@pytest.mark.parametrize("source", ["selection", "blank", "pixels"])
def test_creation_prepares_full_canvas_coverage_before_one_native_commit(
    source: str,
) -> None:
    """Keep channel contents in the native image rather than companion persistence."""
    image, factory, channels, editing = fixture()
    try:
        if source == "selection":
            tattoo = channels.create(
                describe_image(image), "Region", "selection", None, lambda: False
            )
            expected = bytes([127]) * 48
        elif source == "blank":
            tattoo = channels.create(
                describe_image(image), "Region", "blank", None, lambda: False
            )
            expected = bytes(48)
        else:
            expected = bytes(range(48))
            tattoo = channels.create(
                describe_image(image), "Region", "pixels", expected, lambda: False
            )
        channel = factory.created[0]
        assert tattoo == channel.tattoo
        assert image.inserted_channels == [channel]
        assert channel.buffer.data == expected
        assert is_authored_channel(channel)
        assert not channel.visible
        channels.close()
        editing.close()
        assert channel.is_valid() and image.channels[tattoo] is channel
    finally:
        channels.close()
        editing.close()


@pytest.mark.parametrize("after_commit", [False, True])
def test_cancellation_never_denies_an_already_inserted_authored_channel(
    after_commit: bool,
) -> None:
    """Release only prepared native resources when cancellation precedes insertion."""
    image, factory, channels, editing = fixture()
    cancelled = False

    def cancel() -> None:
        """Simulate cancellation at the actual SDK boundary."""
        nonlocal cancelled
        cancelled = True

    factory.on_create = None if after_commit else cancel
    image.on_channel_insert = cancel if after_commit else None
    try:
        if after_commit:
            assert (
                channels.create(
                    describe_image(image), "Region", "blank", None, lambda: cancelled
                )
                == 100
            )
        else:
            with pytest.raises(NativeOperationCancelled):
                channels.create(
                    describe_image(image), "Region", "blank", None, lambda: cancelled
                )
        assert factory.created[0].deleted is not after_commit
        assert len(image.inserted_channels) == int(after_commit)
    finally:
        channels.close()
        editing.close()


def test_editing_survives_native_capture_and_restores_only_the_owned_surface() -> None:
    """Read frozen channel pixels while preserving later native edits and deliberate layer changes."""
    image, factory, channels, editing = fixture()
    snapshots = NativeSnapshots(
        FakeGimpApi(FakeImageLookup({image.image_id: image}), FakeProjectionFactory()),
        NativePixelReader(FakeGeglApi(), 192),
    )
    try:
        document = describe_image(image)
        tattoo = channels.create(document, "Region", "selection", None, lambda: False)
        editing.select(document, tattoo)
        channel = factory.created[0]
        assert image.selected_channels == [channel] and channel.get_visible()
        frozen = snapshots.begin(image.image_id, document.revision)
        channel.name = "Renamed natively"
        channel.buffer.data = bytes([245]) * 48
        assert (
            snapshots.channel(frozen.token, tattoo, 8, 6, 1).data == bytes([127]) * 48
        )
        snapshots.release(frozen.token)
        second = snapshots.begin(image.image_id, document.revision)
        assert (
            snapshots.channel(second.token, tattoo, 8, 6, 1).data == bytes([245]) * 48
        )
        editing.release(tattoo + 1)
        assert channel.get_visible()
        editing.release(tattoo)
        assert not image.selected_channels and not channel.get_visible()
        assert channel.buffer.data == bytes([245]) * 48
        editing.select(document, tattoo)
        another = FakeLayer(FakeBuffer(b""), tattoo=22)
        image.layers.append(another)
        image.set_selected_layers([another])
        channels.close()
        editing.close()
        assert image.selected == [another]
        assert image.channels[tattoo] is channel
    finally:
        snapshots.close()
        channels.close()
        editing.close()


def test_duplicate_channel_tattoos_never_select_another_saved_region() -> None:
    """Retain mask identity through native image copies with independently assigned channel tattoos."""
    image, _, channels, editing = fixture()
    snapshots = NativeSnapshots(
        FakeGimpApi(FakeImageLookup({image.image_id: image}), FakeProjectionFactory()),
        NativePixelReader(FakeGeglApi(), 192),
    )
    try:
        document = describe_image(image)
        first = channels.create(document, "Same name", "selection", None, lambda: False)
        second = channels.create(document, "Same name", "blank", None, lambda: False)

        def renumber(clone: FakeImage) -> None:
            """Model new-image native identities without changing order or metadata."""
            copied: dict[int, FakeChannel] = {}
            for index, channel in enumerate(clone.channels.values()):
                if not isinstance(channel, FakeChannel):
                    raise TypeError("Expected native channel boundary fixture.")
                channel.tattoo = 500 - index
                copied[channel.tattoo] = channel
            clone.channels = dict(copied)

        image.on_duplicate = renumber
        frozen = snapshots.begin(image.image_id, document.revision)
        assert snapshots.channel(frozen.token, first, 8, 6, 1).data == bytes([127]) * 48
        assert snapshots.channel(frozen.token, second, 8, 6, 1).data == bytes(48)
    finally:
        snapshots.close()
        channels.close()
        editing.close()


def test_missing_or_unrelated_channel_is_never_rebound_by_name() -> None:
    """Require the exact persistent authored marker after native deletion or undo."""
    image, factory, channels, editing = fixture()
    try:
        document = describe_image(image)
        tattoo = channels.create(document, "Region", "blank", None, lambda: False)
        del factory.created[0].markers[MASK_MARKER]
        with pytest.raises(StaleNativeImage):
            editing.select(document, tattoo)
        with pytest.raises(StaleNativeImage):
            editing.select(document, tattoo + 1)
        assert not image.selected_channels
    finally:
        channels.close()
        editing.close()


def test_native_display_overlays_are_suppressed_only_on_the_owned_capture_image() -> (
    None
):
    """Keep channel overlays and RGB display filters out of model input without changing GIMP's UI."""
    image, factory, channels, editing = fixture()
    snapshots = NativeSnapshots(
        FakeGimpApi(FakeImageLookup({image.image_id: image}), FakeProjectionFactory()),
        NativePixelReader(FakeGeglApi(), 192),
    )
    try:
        image.components[0] = False
        document = describe_image(image)
        tattoo = channels.create(document, "Region", "selection", None, lambda: False)
        editing.select(document, tattoo)
        snapshots.begin(image.image_id, document.revision)
        clone = image.copies[0]
        assert not clone.get_channels()[0].get_visible()
        assert factory.created[0].get_visible()
        assert clone.components[0]
        assert not image.components[0]
    finally:
        snapshots.close()
        channels.close()
        editing.close()


def test_postcommit_focus_failure_is_observable_without_denying_the_persisted_channel() -> (
    None
):
    """Keep the insertion acknowledgement authoritative if a later native redraw or focus restore fails."""
    image, factory, channels, editing = fixture()
    channels.close()
    errors: list[Exception] = []
    native = NativeChannels(
        FakeChannelApi(FakeChannelLookup(image), factory),
        FakeGeglApi(),
        192,
        errors.append,
    )
    image.fail_layer_select = True
    try:
        assert (
            native.create(describe_image(image), "Region", "blank", None, lambda: False)
            == 100
        )
        assert len(image.inserted_channels) == 1
        assert len(errors) == 1
        assert factory.created[0].is_valid()
    finally:
        native.close()
        editing.close()


def test_failed_preparation_does_not_insert_a_channel() -> None:
    """Delete failed metadata preparation without mutating the original native image."""
    image, factory, channels, editing = fixture()
    factory.fail_marker = True
    try:
        with pytest.raises(RuntimeError, match="metadata"):
            channels.create(
                describe_image(image), "Region", "blank", None, lambda: False
            )
        assert not image.channels and factory.created[0].deleted
    finally:
        channels.close()
        editing.close()
