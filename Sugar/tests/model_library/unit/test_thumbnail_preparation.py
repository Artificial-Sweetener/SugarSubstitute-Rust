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
"""Prove portable model thumbnail geometry, safe addressing, and crop behavior."""

from __future__ import annotations

from io import BytesIO
from pathlib import Path

from PIL import Image
import pytest

from sugarcubes.model_library.domain.banner_crop import choose_banner_crop
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.preview_media import animated_preview
from sugarcubes.model_library.infrastructure import preview_frame


def _source_bytes() -> bytes:
    """Encode a small portrait fixture without external provider access."""
    image = Image.new("RGB", (360, 540), (32, 60, 90))
    stream = BytesIO()
    image.save(stream, "PNG")
    return stream.getvalue()


def test_variants_and_banner_are_reusable_and_decode_in_standard_readers(
    tmp_path: Path,
) -> None:
    """Publish stable dimensions and bytes for browser and future desktop readers."""
    assets = ModelThumbnailAssets(tmp_path)
    preparer = ModelThumbnailPreparer(assets, origin="user")
    references = preparer.prepare(_source_bytes())
    assert [(item.width, item.height) for item in references] == [
        (85, 128),
        (171, 256),
        (341, 512),
        (768, 160),
    ]
    assert [item.role for item in references] == [
        "standard",
        "standard",
        "standard",
        "banner",
    ]
    assert preparer.prepare(_source_bytes()) == references
    assert len(list(tmp_path.glob("*.webp"))) == 4
    assert not list(tmp_path.glob(".pending-*"))
    for reference in references:
        prepared = assets.resolve(reference.asset_id)
        assert prepared is not None
        with Image.open(prepared.path) as image:
            assert image.size == (reference.width, reference.height)
            assert image.format == "WEBP"


def test_invalid_or_missing_assets_cannot_address_arbitrary_files(
    tmp_path: Path,
) -> None:
    """Constrain image serving to opaque prepared IDs under its namespace."""
    assets = ModelThumbnailAssets(tmp_path)
    with pytest.raises(ValueError, match="SHA256"):
        assets.resolve("../../private")
    assert assets.resolve("a" * 64) is None
    with pytest.raises(ValueError, match="WebP"):
        assets.publish(b"not an image")


def test_corrupt_and_oversized_sources_fail_before_publication(tmp_path: Path) -> None:
    """Keep rejected provider responses out of the prepared asset collection."""
    preparer = ModelThumbnailPreparer(ModelThumbnailAssets(tmp_path), origin="user")
    ownership_before = (tmp_path / "sources.sqlite3").read_bytes()
    with pytest.raises(OSError):
        preparer.prepare(b"not a valid image")
    with pytest.raises(ValueError, match="20 MiB"):
        preparer.prepare(b"x" * (20 * 1024 * 1024 + 1))
    assert not list(tmp_path.glob("*.webp"))
    assert (tmp_path / "sources.sqlite3").read_bytes() == ownership_before


def test_banner_policy_prefers_detailed_upper_region_over_flat_background() -> None:
    """Retain Substitute's saliency policy independently of image libraries."""

    def sample(x: int, y: int) -> tuple[int, int, int]:
        """Provide a detailed portrait region over a flat lower background."""
        if y < 90:
            return (255, 60, 10) if (x + y) % 2 else (10, 100, 255)
        return (30, 30, 30)

    rect = choose_banner_crop(150, 224, sample)
    assert rect.width == 150
    assert rect.height == 31
    assert rect.top < 90


@pytest.mark.parametrize("format", ["gif", "mp4", "webm"])
def test_animation_prepares_only_the_first_frame_as_static_variants(
    tmp_path: Path, format: str
) -> None:
    """Keep animation frames out of portable thumbnails and cache identical source bytes."""
    assets = ModelThumbnailAssets(tmp_path)
    preparer = ModelThumbnailPreparer(assets, origin="civitai")
    content = animated_preview(format)
    references = preparer.prepare(content)
    assert preparer.prepare(content) == references
    assert len(references) == 4
    for reference in references:
        prepared = assets.resolve(reference.asset_id)
        assert prepared is not None
        with Image.open(prepared.path) as image:
            assert not getattr(image, "is_animated", False)
            pixel = image.convert("RGB").getpixel((0, 0))
            assert isinstance(pixel, tuple)
            assert pixel[0] > 230 and pixel[1] < 20 and pixel[2] < 20


@pytest.mark.parametrize(
    "content", [b"\x00\x00\x00\x20ftypbroken", b"\x1a\x45\xdf\xa3broken"]
)
def test_corrupt_video_does_not_publish_thumbnail_assets(
    tmp_path: Path, content: bytes
) -> None:
    """Surface decoder failures through the existing preview error boundary."""
    preparer = ModelThumbnailPreparer(ModelThumbnailAssets(tmp_path), origin="civitai")
    with pytest.raises(ValueError, match="could not be decoded"):
        preparer.prepare(content)
    assert not list(tmp_path.glob("*.webp"))


@pytest.mark.parametrize("format", ["gif", "mp4", "webm"])
def test_visual_pixel_limit_applies_to_animated_sources(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, format: str
) -> None:
    """Reject oversized animation frames before allocating thumbnail variants."""
    monkeypatch.setattr(preview_frame, "_PIXEL_LIMIT", 100)
    preparer = ModelThumbnailPreparer(ModelThumbnailAssets(tmp_path), origin="civitai")
    with pytest.raises(ValueError, match="pixel limit"):
        preparer.prepare(animated_preview(format))
    assert not list(tmp_path.glob("*.webp"))
