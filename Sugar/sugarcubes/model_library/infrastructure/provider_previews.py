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
        """Decode a bounded provider image away from the Comfy HTTP event loop.

        Raises:
            ModelPreviewUnavailableError: The provider image could not be downloaded, which is
                an expected outcome, or could not be prepared, which is not.
        """
        provider, images = (
            (self._openmodeldb, self._openmodeldb_images)
            if approved_openmodeldb_preview(url)
            else (self._civitai, self._civitai_images)
        )
        try:
            content = await provider.read_preview(url)
        except (aiohttp.ClientError, TimeoutError, ValueError) as error:
            raise ModelPreviewUnavailableError(
                "The provider preview could not be downloaded.",
                reason=transport_failure_reason(error),
                expected=True,
            ) from error
        except OSError as error:
            raise ModelPreviewUnavailableError(
                "The provider preview could not be downloaded.",
                reason=type(error).__name__,
                expected=False,
            ) from error
        try:
            return await complete_blocking_model_work(images.prepare, content)
        except (OSError, ValueError, sqlite3.Error) as error:
            raise ModelPreviewUnavailableError(
                "The provider preview could not be prepared.",
                reason=type(error).__name__,
                expected=False,
            ) from error


def transport_failure_reason(error: Exception) -> str:
    """Describe an expected provider transport failure in plain words, without a traceback."""
    if isinstance(error, TimeoutError):
        return "the provider did not answer in time"
    if isinstance(error, aiohttp.ClientConnectorError):
        return f"could not connect to {error.host}"
    if isinstance(error, aiohttp.ClientResponseError):
        return f"the provider answered HTTP {error.status}"
    if isinstance(error, aiohttp.ClientPayloadError):
        return "the provider response ended early"
    if isinstance(error, aiohttp.ClientError):
        return "the connection to the provider failed"
    return str(error) or type(error).__name__
