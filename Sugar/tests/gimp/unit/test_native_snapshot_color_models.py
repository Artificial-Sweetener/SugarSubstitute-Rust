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
"""Keep non-RGB source capture independent of native component-visibility limitations."""

from __future__ import annotations

import pytest

from integrations.gimp.native.images import describe_image
from integrations.gimp.native.pixels import NativePixelReader
from integrations.gimp.native.snapshots import NativeSnapshots
from tests.gimp.support.native_images import (
    FakeBaseTypes,
    FakeComponentTypes,
    FakeGeglApi,
    FakeGimpApi,
    FakeImage,
    FakeImageLookup,
    FakeProjectionFactory,
)


@pytest.mark.parametrize("base_type", [FakeBaseTypes.GRAY, FakeBaseTypes.INDEXED])
def test_non_rgb_capture_normalizes_only_the_owned_snapshot(base_type: int) -> None:
    """Capture color and selection without applying RGB-only alpha visibility to the source."""
    image = FakeImage(base_type=base_type, components={FakeComponentTypes.ALPHA: False})
    original = describe_image(image)
    projection = FakeProjectionFactory()
    snapshots = NativeSnapshots(
        FakeGimpApi(FakeImageLookup({image.image_id: image}), projection),
        NativePixelReader(FakeGeglApi(), 192),
    )
    try:
        snapshot = snapshots.begin(image.image_id, original.revision)
        frozen = image.copies[0]
        assert snapshot.document == original
        assert image.base_type == base_type
        assert image.rgb_conversions == 0
        assert image.components == {FakeComponentTypes.ALPHA: False}
        assert frozen.base_type == FakeBaseTypes.RGB
        assert frozen.rgb_conversions == 1
        assert all(frozen.components[component] for component in (0, 1, 2, 3))
        assert (
            snapshots.projection(snapshot.token, [], 8, 6, 1).data == image.projection
        )
        assert snapshots.selection(snapshot.token, 8, 6, 1).data == bytes([127]) * 48
        assert describe_image(image) == original
    finally:
        snapshots.close()
    assert image.delete_calls == 0
    assert image.copies[0].delete_calls == 1


@pytest.mark.parametrize("base_type", [FakeBaseTypes.GRAY, FakeBaseTypes.INDEXED])
def test_failed_color_normalization_releases_the_copy_and_preserves_the_source(
    base_type: int,
) -> None:
    """Fail closed without leaking a native copy when canonical color preparation fails."""
    image = FakeImage(base_type=base_type, fail_rgb_conversion=True)
    projection = FakeProjectionFactory()
    snapshots = NativeSnapshots(
        FakeGimpApi(FakeImageLookup({image.image_id: image}), projection),
        NativePixelReader(FakeGeglApi(), 192),
    )
    try:
        with pytest.raises(RuntimeError, match="frozen image to RGB"):
            snapshots.begin(image.image_id, describe_image(image).revision)
        assert image.base_type == base_type
        assert image.rgb_conversions == 0
        assert image.delete_calls == 0
        assert image.copies[0].delete_calls == 1
        assert projection.created == []
    finally:
        snapshots.close()
