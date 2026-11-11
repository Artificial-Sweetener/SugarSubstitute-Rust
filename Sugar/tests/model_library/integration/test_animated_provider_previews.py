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
"""Prove provider animations flow through transport into cached static WebP thumbnail variants."""

import asyncio
from pathlib import Path

import aiohttp
from PIL import Image
import pytest

from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from sugarcubes.model_library.infrastructure.openmodeldb_client import (
    OpenModelDbProvider,
)
from sugarcubes.model_library.infrastructure.provider_catalog_cache import (
    ProviderCatalogCache,
)
from sugarcubes.model_library.infrastructure.provider_previews import (
    ModelProviderPreviewPreparer,
)
from sugarcubes.model_library.infrastructure.thumbnail_assets import (
    ModelThumbnailAssets,
)
from sugarcubes.model_library.infrastructure.thumbnail_preparer import (
    ModelThumbnailPreparer,
)
from tests.model_library.support.preview_media import animated_preview
from tests.model_library.support.provider_transport import (
    TransportResponse,
    TransportSession,
)


@pytest.mark.parametrize(
    "format,mime", [("gif", "image/gif"), ("mp4", "video/mp4"), ("webm", "video/webm")]
)
def test_provider_animation_becomes_a_cached_first_frame(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, format: str, mime: str
) -> None:
    """Exercise production preparation while replacing only the remote HTTP response."""
    session = TransportSession(TransportResponse(200, animated_preview(format), mime))
    monkeypatch.setattr(aiohttp, "ClientSession", session.factory)
    assets = ModelThumbnailAssets(tmp_path / "assets")
    previews = ModelProviderPreviewPreparer(
        CivitaiModelProvider(),
        OpenModelDbProvider(ProviderCatalogCache(tmp_path / "openmodeldb.json")),
        ModelThumbnailPreparer(assets, origin="civitai"),
        ModelThumbnailPreparer(assets, origin="openmodeldb"),
    )
    references = asyncio.run(previews.prepare("https://image.civitai.com/animation"))
    assert previews.available(references)
    assert len(references) == 4
    for reference in references:
        asset = assets.resolve(reference.asset_id)
        assert asset is not None and asset.content_type == "image/webp"
        with Image.open(asset.path) as image:
            assert not getattr(image, "is_animated", False)
            pixel = image.convert("RGB").getpixel((0, 0))
            assert isinstance(pixel, tuple) and pixel[0] > 230 and pixel[2] < 20
