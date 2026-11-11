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
"""Prepare portable thumbnail variants and Substitute-style banners without Qt."""

from __future__ import annotations

from io import BytesIO
import hashlib

from PIL import Image, ImageOps

from ..domain.banner_crop import choose_banner_crop
from ..domain.models import ThumbnailReference
from ..domain.thumbnail_sources import ThumbnailSourceOrigin, thumbnail_source_origin
from .thumbnail_assets import ModelThumbnailAssets
from .preview_frame import first_preview_frame

_PREPARATION_POLICY = b"portable-model-thumbnail-v1\0"


class ModelThumbnailPreparer:
    """Bound source-image decoding and publish reusable standard/banner variants."""

    def __init__(
        self, assets: ModelThumbnailAssets, *, origin: ThumbnailSourceOrigin
    ) -> None:
        """Bind the prepared-asset owner independently from provider transport."""
        self._assets = assets
        self._origin = thumbnail_source_origin(origin)

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Check every selected prepared variant without decoding image payloads."""
        return all(
            self._assets.resolve(item.asset_id) is not None for item in thumbnails
        )

    def prepare(self, content: bytes) -> tuple[ThumbnailReference, ...]:
        """Generate cached image variants without loading any model weights."""
        if not content or len(content) > 20 * 1024 * 1024:
            raise ValueError("Model preview must be between 1 byte and 20 MiB.")
        return self._assets.prepare_source(
            self._origin,
            hashlib.sha256(_PREPARATION_POLICY + content).hexdigest(),
            lambda: self._prepare(content),
        )

    def _prepare(self, content: bytes) -> tuple[ThumbnailReference, ...]:
        """Decode and publish variants while holding the source owner's collection lease."""
        source = first_preview_frame(content)
        variants: list[ThumbnailReference] = []
        for size in (128, 256, 512):
            image = source.copy()
            image.thumbnail((size, size), Image.Resampling.LANCZOS)
            variants.append(self._publish(image, "standard"))
        analysis = source.copy()
        analysis.thumbnail((224, 224), Image.Resampling.LANCZOS)
        rect = choose_banner_crop(
            analysis.width, analysis.height, lambda x, y: _rgb(analysis, x, y)
        )
        left = round(rect.left * source.width / analysis.width)
        top = round(rect.top * source.height / analysis.height)
        width = min(
            source.width, max(1, round(rect.width * source.width / analysis.width))
        )
        height = min(
            source.height, max(1, round(rect.height * source.height / analysis.height))
        )
        left = min(left, source.width - width)
        top = min(top, source.height - height)
        cropped = source.crop((left, top, left + width, top + height))
        banner = ImageOps.fit(cropped, (768, 160), Image.Resampling.LANCZOS)
        variants.append(self._publish(banner, "banner"))
        return tuple(variants)

    def _publish(self, image: Image.Image, role: str) -> ThumbnailReference:
        """Encode deterministic browser/Qt-readable assets with known geometry."""
        stream = BytesIO()
        image.save(stream, format="WEBP", quality=90, method=4)
        asset_id = self._assets.publish(stream.getvalue())
        return ThumbnailReference(
            asset_id, role, image.width, image.height, "image/webp"
        )


def _rgb(image: Image.Image, x: int, y: int) -> tuple[int, int, int]:
    """Narrow Pillow's pixel union at the image adapter boundary."""
    pixel: object = image.getpixel((x, y))
    if (
        not isinstance(pixel, tuple)
        or len(pixel) != 3
        or not all(isinstance(channel, int) for channel in pixel)
    ):
        raise ValueError("Expected an RGB thumbnail analysis image.")
    red, green, blue = pixel
    if (
        not isinstance(red, int)
        or not isinstance(green, int)
        or not isinstance(blue, int)
    ):
        raise ValueError("Invalid RGB analysis pixel.")
    return red, green, blue
