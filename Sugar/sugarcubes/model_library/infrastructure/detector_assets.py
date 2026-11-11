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
"""Prepare Substitute's packaged detector art once under the shared model root."""

from __future__ import annotations

from io import BytesIO
import hashlib
from pathlib import Path

from PIL import Image

from ..domain.detector_visuals import detector_library
from ..domain.models import ThumbnailReference
from .thumbnail_assets import ModelThumbnailAssets

_PREPARATION_POLICY = b"detector-illustration-v1\0"


class BundledDetectorAssets:
    """Publish square visuals and the reference's dark banner wash without saliency cropping."""

    def __init__(self, assets: ModelThumbnailAssets) -> None:
        """Bind immutable shared storage while keeping source assets package-local."""
        self._assets = assets
        self._allowed = frozenset(item.asset_name for item in detector_library())

    def variants(self, name: str) -> tuple[ThumbnailReference, ...]:
        """Resolve only curated names and preserve identical illustration geometry across clients."""
        if name not in self._allowed:
            raise ValueError("Unknown detector illustration.")
        path = Path(__file__).parent / "resources" / "detectors" / f"{name}.png"
        content = path.read_bytes()
        return self._assets.prepare_source(
            "bundled",
            hashlib.sha256(_PREPARATION_POLICY + content).hexdigest(),
            lambda: self._prepare(content),
        )

    def _prepare(self, content: bytes) -> tuple[ThumbnailReference, ...]:
        """Publish curated variants under the shared source collection lease."""
        with Image.open(BytesIO(content)) as opened:
            source = opened.convert("RGBA")
        wash = Image.new("RGBA", source.size, (48, 48, 48, 104))
        banner = Image.alpha_composite(source, wash)
        return tuple(
            self._publish(image, role)
            for image, role in ((source, "standard"), (banner, "banner"))
        )

    def _publish(self, image: Image.Image, role: str) -> ThumbnailReference:
        """Encode transport-neutral content-addressed images for the browser and desktop host."""
        stream = BytesIO()
        image.save(stream, format="WEBP", lossless=True, method=4)
        return ThumbnailReference(
            self._assets.publish(stream.getvalue()),
            role,
            image.width,
            image.height,
            "image/webp",
        )
