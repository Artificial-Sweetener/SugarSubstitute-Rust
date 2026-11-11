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
"""Prove native raster admission before expensive SDK pixel reads."""

from __future__ import annotations

from typing import Literal

import pytest

from integrations.gimp.native.model import NativeDocument
from integrations.gimp.native.pixels import NativePixelReader
from tests.gimp.support.native_images import FakeBuffer, FakeDrawable, FakeGeglApi


@pytest.mark.parametrize(
    "format_name, channels, native_format",
    [("rgba8", 4, "R'G'B'A u8"), ("coverage8", 1, "Y u8")],
)
def test_native_scaled_roi_preserves_half_pixel_rounding_and_requests_explicit_formats(
    format_name: Literal["rgba8", "coverage8"], channels: int, native_format: str
) -> None:
    """Keep GEGL's scaled coordinate convention and exact canonical bytes."""
    data = bytes(range(3 * channels))
    buffer = FakeBuffer(data)
    reader = NativePixelReader(FakeGeglApi(), 12)
    pixels = reader.read(
        FakeDrawable(buffer),
        NativeDocument(1, "geometry", 5, 2, "Image"),
        3,
        1,
        0.5,
        format_name,
    )
    assert (pixels.width, pixels.height, pixels.format, pixels.data) == (
        3,
        1,
        format_name,
        data,
    )
    assert buffer.calls == [((0, 0, 3, 1), 0.5, native_format, 1)]


@pytest.mark.parametrize("scale", [0, -1, 1.01, float("nan"), float("inf")])
def test_invalid_native_scales_never_reach_the_sdk(scale: float) -> None:
    """Refuse nonfinite or enlarging requests at the native boundary."""
    buffer = FakeBuffer(b"")
    with pytest.raises(ValueError, match="scale"):
        NativePixelReader(FakeGeglApi(), 192).read(
            FakeDrawable(buffer),
            NativeDocument(1, "geometry", 8, 6, "Image"),
            8,
            6,
            scale,
            "rgba8",
        )
    assert buffer.calls == []


@pytest.mark.parametrize("nonlinear,native_format", [(False, "Y u8"), (True, "Y' u8")])
def test_native_channel_coverage_matches_gimps_raw_copy_format_exception(
    nonlinear: bool, native_format: str
) -> None:
    """Preserve coverage samples rather than gamma-converting GIMP's legacy 8-bit channels."""
    data = bytes(range(256))
    buffer = FakeBuffer(data)
    reader = NativePixelReader(FakeGeglApi(), 256)
    pixels = reader.read_channel(
        FakeDrawable(buffer),
        NativeDocument(1, "geometry", 16, 16, "Image"),
        16,
        16,
        1,
        nonlinear,
    )
    assert pixels.data == data
    assert buffer.calls[0][2] == native_format


def test_geometry_and_byte_budget_are_admitted_before_native_allocation() -> None:
    """Avoid requesting a native allocation for inconsistent or over-budget geometry."""
    buffer = FakeBuffer(b"")
    reader = NativePixelReader(FakeGeglApi(), 191)
    document = NativeDocument(1, "geometry", 8, 6, "Image")
    with pytest.raises(ValueError, match="dimensions"):
        reader.read(FakeDrawable(buffer), document, 9, 6, 1, "rgba8")
    with pytest.raises(ValueError, match="budget"):
        reader.read(FakeDrawable(buffer), document, 8, 6, 1, "rgba8")
    assert buffer.calls == []


def test_tiny_previews_retain_one_pixel_and_reject_malformed_native_output() -> None:
    """Keep tiny canvases representable while checking exact output packing."""
    buffer = FakeBuffer(b"\x00\x01\x02\xff")
    reader = NativePixelReader(FakeGeglApi(), 4)
    document = NativeDocument(1, "geometry", 1, 1, "Image")
    assert (
        reader.read(FakeDrawable(buffer), document, 1, 1, 0.1, "rgba8").data
        == buffer.data
    )
    buffer.data = b""
    with pytest.raises(RuntimeError, match="byte count"):
        reader.read(FakeDrawable(buffer), document, 1, 1, 1, "rgba8")
