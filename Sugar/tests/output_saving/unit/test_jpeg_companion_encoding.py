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
"""Prove companion compression keeps image geometry and strips recipe metadata."""

from io import BytesIO

from PIL import Image

from sugarcubes.output_saving.domain.preferences import JpegPreferences, JpegSizingMode
from sugarcubes.output_saving.infrastructure.jpeg_encoder import JpegCompanionEncoder


def test_fixed_quality_flattens_transparency_without_recipe_metadata() -> None:
    """Preserve Substitute's white transparency background and shareable representation."""
    image = Image.new("RGBA", (20, 10), (255, 0, 0, 0))
    image.info["workflow"] = "private recipe"
    image.info["sugar_script"] = "private script"
    encoded = JpegCompanionEncoder().encode(image, JpegPreferences(enabled=True))
    with Image.open(BytesIO(encoded)) as decoded:
        assert decoded.format == "JPEG"
        assert decoded.size == (20, 10)
        assert decoded.mode == "RGB"
        assert decoded.getpixel((0, 0)) == (255, 255, 255)
        assert "workflow" not in decoded.info
        assert "sugar_script" not in decoded.info


def test_target_size_changes_quality_without_resizing() -> None:
    """Meet the target when attainable while retaining full output dimensions."""
    image = Image.effect_noise((128, 96), 100).convert("RGB")
    settings = JpegPreferences(
        enabled=True, sizing_mode=JpegSizingMode.TARGET_SIZE, target_size_kib=4
    )
    encoded = JpegCompanionEncoder().encode(image, settings)
    assert len(encoded) <= 4 * 1024
    with Image.open(BytesIO(encoded)) as decoded:
        assert decoded.size == image.size


def test_impossible_target_returns_smallest_quality_without_losing_the_image() -> None:
    """Honor approximate size semantics when JPEG overhead exceeds the requested target."""
    image = Image.new("RGB", (4096, 1), (128, 128, 128))
    settings = JpegPreferences(
        enabled=True, sizing_mode=JpegSizingMode.TARGET_SIZE, target_size_kib=1
    )
    encoded = JpegCompanionEncoder().encode(image, settings)
    with Image.open(BytesIO(encoded)) as decoded:
        assert decoded.size == image.size
