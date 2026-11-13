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
"""Prove coherent native capture lifetime and cleanup at the SDK boundary."""

from __future__ import annotations

import pytest

from integrations.gimp.native.images import describe_image, resolve_image
from integrations.gimp.native.layers import layer_catalog
from integrations.gimp.native.model import StaleNativeImage
from integrations.gimp.native.pixels import NativePixelReader
from integrations.gimp.native.snapshots import NativeSnapshots, NativeVisibility
from tests.gimp.support.native_images import (
    FakeBuffer,
    FakeGeglApi,
    FakeGimpApi,
    FakeImage,
    FakeImageLookup,
    FakeLayer,
    FakeProjectionFactory,
)


def fixture() -> tuple[FakeImage, FakeProjectionFactory, NativeSnapshots]:
    """Compose the production capture owner with typed native boundaries."""
    first = FakeLayer(FakeBuffer(b""), tattoo=1, name="<literal layer>")
    second = FakeLayer(FakeBuffer(b""), tattoo=2, name="Hidden", visible=False)
    image = FakeImage(layers=[first, second], selected=[second])
    projection = FakeProjectionFactory()
    gimp = FakeGimpApi(FakeImageLookup({image.image_id: image}), projection)
    return (
        image,
        projection,
        NativeSnapshots(gimp, NativePixelReader(FakeGeglApi(), 192)),
    )


def test_native_geometry_qualifies_color_and_shape_without_following_content_or_names() -> (
    None
):
    """Preserve destination coordinates across content edits while detecting reinterpretation."""
    image = FakeImage()
    original = describe_image(image)
    image.name = "Renamed"
    image.projection = b"edited pixels"
    assert describe_image(image).revision == original.revision
    for field, value in [
        ("width", 9),
        ("precision", 16),
        ("profile", b"another profile"),
    ]:
        changed = FakeImage()
        setattr(changed, field, value)
        assert describe_image(changed).revision != original.revision


def test_metadata_read_rejects_a_resize_instead_of_returning_mixed_dimensions() -> None:
    """Fail closed when native geometry changes between SDK responses."""
    with pytest.raises(StaleNativeImage, match="metadata"):
        describe_image(FakeImage(change_width_at=2))


def test_missing_and_retained_invalid_proxies_are_never_substituted() -> None:
    """Require native validity in addition to the existence of a Python proxy."""
    closed = FakeImage(valid=False)
    lookup = FakeImageLookup({1: closed, 2: FakeImage(image_id=2)})
    for image_id in (1, 3):
        with pytest.raises(StaleNativeImage, match="closed"):
            resolve_image(lookup, image_id)
    assert closed.copies == []


def test_snapshot_freezes_pixels_selection_and_layer_metadata_until_release() -> None:
    """Keep one batch coherent while the original remains independently editable."""
    image, factory, snapshots = fixture()
    info = snapshots.begin(image.image_id, describe_image(image).revision)
    image.projection = bytes([0, 0, 255, 255]) * 48
    image.layers[0].name = "Edited layer"
    image.selection.buffer.data = bytes(48)
    assert info.primary_layer == 2
    assert info.layers[0].name == "<literal layer>"
    assert (
        snapshots.projection(info.token, [], 8, 6, 1).data
        == bytes([10, 200, 30, 255]) * 48
    )
    assert snapshots.selection(info.token, 8, 6, 1).data == bytes([127]) * 48
    assert factory.created[0].deleted
    snapshots.release(info.token)
    assert image.copies[0].delete_calls == 1
    assert image.delete_calls == 0
    assert image.valid
    with pytest.raises(StaleNativeImage, match="snapshot"):
        snapshots.selection(info.token, 8, 6, 1)


def test_frozen_layer_tattoos_are_mapped_to_the_original_catalog() -> None:
    """Preserve source identities when GIMP renumbers duplicated items."""
    image, factory, snapshots = fixture()

    def renumber(clone: FakeImage) -> None:
        """Model GIMP's new-image tattoo allocation at its SDK boundary."""
        clone.layers[0].tattoo = 202
        clone.layers[1].tattoo = 201

    image.on_duplicate = renumber
    try:
        info = snapshots.begin(image.image_id, describe_image(image).revision)
        assert [layer.tattoo for layer in info.layers] == [1, 2]
        assert info.primary_layer == 2
        snapshots.projection(info.token, [NativeVisibility(2, True)], 8, 6, 1)
        assert factory.visibility_reads == [[(202, True), (201, True)]]
        assert [layer.tattoo for layer in image.layers] == [1, 2]
        assert not image.layers[1].visible
    finally:
        snapshots.close()


def test_topology_changes_during_duplication_refuse_admission_and_release_the_copy() -> (
    None
):
    """Reject reordered source identities rather than pairing a captured layer with another source."""
    image, _, snapshots = fixture()
    image.on_duplicate = lambda clone: image.layers.reverse()
    try:
        with pytest.raises(StaleNativeImage, match="catalog"):
            snapshots.begin(image.image_id, describe_image(image).revision)
        assert image.copies[0].delete_calls == 1
        assert image.delete_calls == 0
    finally:
        snapshots.close()


def test_projection_failure_restores_visibility_and_can_retry_the_same_snapshot() -> (
    None
):
    """Retire temporary state even when the external native renderer fails."""
    image, factory, snapshots = fixture()
    info = snapshots.begin(image.image_id, describe_image(image).revision)
    factory.fail = True
    with pytest.raises(RuntimeError, match="projection failed"):
        snapshots.projection(
            info.token, [NativeVisibility(1, False), NativeVisibility(2, True)], 8, 6, 1
        )
    assert [layer.visible for layer in image.copies[0].layers] == [True, False]
    assert [layer.visible for layer in image.layers] == [True, False]
    factory.fail = False
    assert snapshots.projection(info.token, [], 8, 6, 1).width == 8
    snapshots.close()


def test_bad_native_bytes_delete_the_projection_and_restore_frozen_visibility() -> None:
    """Reject malformed external output before returning a portable raster."""
    image, factory, snapshots = fixture()
    info = snapshots.begin(image.image_id, describe_image(image).revision)
    image.copies[0].projection = b"wrong size"
    with pytest.raises(RuntimeError, match="byte count"):
        snapshots.projection(info.token, [NativeVisibility(1, False)], 8, 6, 1)
    assert factory.created[0].deleted
    assert image.copies[0].layers[0].visible
    snapshots.close()


def test_stale_geometry_and_failed_freezing_release_only_owned_images() -> None:
    """Avoid retaining a temporary image when a native snapshot cannot be admitted."""
    image, _, snapshots = fixture()
    revision = describe_image(image).revision
    image.width += 1
    with pytest.raises(StaleNativeImage):
        snapshots.begin(image.image_id, revision)
    assert image.copies == []
    image.on_duplicate = lambda clone: setattr(clone, "width", clone.width + 1)
    with pytest.raises(StaleNativeImage):
        snapshots.begin(image.image_id, describe_image(image).revision)
    assert image.copies[0].delete_calls == 1
    assert image.delete_calls == 0


def test_close_attempts_every_owned_snapshot_and_refuses_later_allocations() -> None:
    """Report cleanup failures without abandoning other batches or touching source images."""
    image, _, snapshots = fixture()
    revision = describe_image(image).revision
    for _ in range(4):
        snapshots.begin(image.image_id, revision)
    with pytest.raises(RuntimeError, match="limit"):
        snapshots.begin(image.image_id, revision)
    image.copies[0].fail_delete = True
    with pytest.raises(ExceptionGroup, match="shutdown cleanup"):
        snapshots.close()
    assert [copy.delete_calls for copy in image.copies] == [1, 1, 1, 1]
    assert image.delete_calls == 0
    with pytest.raises(StaleNativeImage, match="closed"):
        snapshots.begin(image.image_id, revision)
    image.copies[0].fail_delete = False
    snapshots.close()
    assert [copy.delete_calls for copy in image.copies] == [2, 1, 1, 1]


def test_failed_native_projection_release_stays_owned_until_cleanup_can_retry() -> None:
    """Do not forget a native resource merely because its first deletion failed."""
    image, factory, snapshots = fixture()
    info = snapshots.begin(image.image_id, describe_image(image).revision)
    factory.fail_delete = True
    with pytest.raises(RuntimeError, match="projection layer"):
        snapshots.projection(info.token, [], 8, 6, 1)
    with pytest.raises(ExceptionGroup, match="projection cleanup"):
        snapshots.release(info.token)
    with pytest.raises(StaleNativeImage, match="snapshot"):
        snapshots.projection(info.token, [], 8, 6, 1)
    assert len(factory.created) == 1
    assert image.copies[0].delete_calls == 0
    projection = factory.created[0]
    projection.fail_delete = False
    snapshots.release(info.token)
    assert projection.deleted
    assert projection.delete_calls == 3
    assert image.copies[0].delete_calls == 1
    assert image.delete_calls == 0
    snapshots.close()


def test_unadmitted_snapshot_remains_owned_when_rollback_deletion_fails() -> None:
    """Retain cleanup ownership even when a native duplicate cannot become a valid batch."""
    image, _, snapshots = fixture()

    def invalidate(clone: FakeImage) -> None:
        """Fail snapshot admission and the first native rollback."""
        clone.width += 1
        clone.fail_delete = True

    image.on_duplicate = invalidate
    with pytest.raises(RuntimeError, match="release capture image"):
        snapshots.begin(image.image_id, describe_image(image).revision)
    clone = image.copies[0]
    assert clone.delete_calls == 1
    clone.fail_delete = False
    snapshots.close()
    assert clone.delete_calls == 2
    assert not clone.valid
    assert image.delete_calls == 0


def test_layer_catalog_is_preorder_and_rejects_duplicate_or_cyclic_native_identities() -> (
    None
):
    """Keep hierarchy discovery separate from shared visibility-range decisions."""
    child = FakeLayer(FakeBuffer(b""), tattoo=2)
    parent = FakeLayer(FakeBuffer(b""), tattoo=1, children=[child])
    image = FakeImage(layers=[parent])
    assert [
        (layer.tattoo, layer.parent_tattoo, layer.depth)
        for layer in layer_catalog(image)
    ] == [(1, None, 0), (2, 1, 1)]
    child.children = [parent]
    with pytest.raises(RuntimeError, match="duplicate"):
        layer_catalog(image)


def test_unknown_visibility_target_does_not_modify_the_frozen_stack() -> None:
    """Validate all external target identities before mutating any temporary layer."""
    image, factory, snapshots = fixture()
    info = snapshots.begin(image.image_id, describe_image(image).revision)
    with pytest.raises(StaleNativeImage, match="unavailable"):
        snapshots.projection(
            info.token,
            [NativeVisibility(1, False), NativeVisibility(99, True)],
            8,
            6,
            1,
        )
    assert image.copies[0].layers[0].visible
    assert factory.created == []
    snapshots.close()


def test_invalid_capture_geometry_never_creates_a_native_projection() -> None:
    """Reject bad allocation requests before changing any frozen native layer state."""
    image, factory, snapshots = fixture()
    info = snapshots.begin(image.image_id, describe_image(image).revision)
    with pytest.raises(ValueError, match="dimensions"):
        snapshots.projection(info.token, [NativeVisibility(1, False)], 8000, 6000, 1)
    assert factory.created == []
    assert image.copies[0].layers[0].visible
    snapshots.close()
