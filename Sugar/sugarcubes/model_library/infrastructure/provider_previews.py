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
"""Connect provider image transport to the portable prepared-asset pipeline."""

from __future__ import annotations

import sqlite3

import aiohttp

from ..application.enrichment_service import ModelPreviewUnavailableError
from ..application.blocking_model_work import complete_blocking_model_work
from ..domain.models import ThumbnailReference
from .civitai_client import CivitaiModelProvider
from .thumbnail_preparer import ModelThumbnailPreparer
from .openmodeldb_client import OpenModelDbProvider
from .openmodeldb_codec import approved_openmodeldb_preview


class ModelProviderPreviewPreparer:
    """Keep network and image preparation off the model-picker interaction path."""

    def __init__(
        self,
        civitai: CivitaiModelProvider,
        openmodeldb: OpenModelDbProvider,
        civitai_images: ModelThumbnailPreparer,
        openmodeldb_images: ModelThumbnailPreparer,
    ) -> None:
        """Use the same provider transport and immutable image owner as other library jobs."""
        self._civitai = civitai
        self._openmodeldb = openmodeldb
        self._civitai_images = civitai_images
        self._openmodeldb_images = openmodeldb_images

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Delegate prepared-image freshness to the same owner that publishes variants."""
        return self._civitai_images.available(thumbnails)

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Decode a bounded provider image away from the Comfy HTTP event loop."""
        try:
            provider, images = (
                (self._openmodeldb, self._openmodeldb_images)
                if approved_openmodeldb_preview(url)
                else (self._civitai, self._civitai_images)
            )
            content = await provider.read_preview(url)
            return await complete_blocking_model_work(images.prepare, content)
        except (
            aiohttp.ClientError,
            TimeoutError,
            OSError,
            ValueError,
            sqlite3.Error,
        ) as error:
            raise ModelPreviewUnavailableError(
                "The provider preview could not be prepared."
            ) from error
