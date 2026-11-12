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
"""Prove cancelled provider preparation drains physical publication before returning."""

from __future__ import annotations

import asyncio
from io import BytesIO
from pathlib import Path
from threading import Event

from PIL import Image
import pytest

from sugarcubes.model_library.domain.models import ThumbnailReference
from sugarcubes.model_library.infrastructure.civitai_client import CivitaiModelProvider
from sugarcubes.model_library.infrastructure.civitai_transport import CivitaiTransport
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
from tests.support.no_progress import wait_for_event


class PausedImages(ModelThumbnailPreparer):
    """Hold actual source-ledger and WebP publication at the blocking adapter boundary."""

    def __init__(self, assets: ModelThumbnailAssets) -> None:
        """Expose bounded worker signals independently of the caller's asyncio task."""
        super().__init__(assets, origin="civitai")
        self.started = Event()
        self.release = Event()
        self.finished = Event()

    def prepare(self, content: bytes) -> tuple[ThumbnailReference, ...]:
        """Publish real prepared files only after the proof releases the worker."""
        self.started.set()
        try:
            wait_for_event(self.release, "Preview proof did not release publication.")
            return super().prepare(content)
        finally:
            self.finished.set()


def test_cancelled_preparation_remains_active_until_publication_finishes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Keep repeated caller cancellation from disguising a still-running image writer."""
    image = BytesIO()
    Image.new("RGB", (24, 32), "teal").save(image, "PNG")

    async def read_preview(_self: CivitaiTransport, _url: str) -> bytes:
        """Replace only external transport while preserving actual image preparation."""
        return image.getvalue()

    monkeypatch.setattr(CivitaiTransport, "read_preview", read_preview)
    assets = ModelThumbnailAssets(tmp_path / "assets")
    images = PausedImages(assets)
    preparer = ModelProviderPreviewPreparer(
        CivitaiModelProvider(),
        OpenModelDbProvider(ProviderCatalogCache(tmp_path / "openmodeldb.json")),
        images,
        ModelThumbnailPreparer(assets, origin="openmodeldb"),
    )

    async def exercise() -> None:
        """Observe the production preparation task and a real independent blocking writer."""
        task = asyncio.create_task(
            preparer.prepare("https://image.civitai.com/fixture.jpg")
        )
        try:
            await asyncio.to_thread(wait_for_event, images.started)
            task.cancel()
            await asyncio.sleep(0)
            task.cancel()
            await asyncio.sleep(0)
            assert (
                not task.done()
            ), "Cancellation returned while the physical image writer was still active."
        finally:
            images.release.set()
            await asyncio.gather(task, return_exceptions=True)
            await asyncio.to_thread(wait_for_event, images.finished)
        assert task.cancelled()
        assert assets.source_summary("civitai").source_count == 1

    asyncio.run(exercise())
