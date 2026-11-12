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
"""Resolve compatible version previews independently of model choice and acquisition."""

from __future__ import annotations

import asyncio
import logging

from .release_service import ModelReleaseService
from .enrichment_service import ModelPreviewPreparer
from .blocking_model_work import complete_blocking_model_work
from .metadata_activity import ModelMetadataActivity
from ..domain.models import ThumbnailReference

_LOGGER = logging.getLogger(__name__)


class ModelReleasePreviews:
    """Coalesce bounded image preparation while delegating URL approval to the provider adapter."""

    def __init__(
        self,
        releases: ModelReleaseService,
        previews: ModelPreviewPreparer,
        activity: ModelMetadataActivity,
    ) -> None:
        """Reuse the authoritative family and prepared-image owners."""
        self._releases = releases
        self._previews = previews
        self._activity = activity
        self._pending: dict[str, asyncio.Task[tuple[ThumbnailReference, ...]]] = {}
        self._prepared: dict[str, tuple[ThumbnailReference, ...]] = {}

    async def get(
        self, model_id: str, version_id: str
    ) -> tuple[ThumbnailReference, ...]:
        """Resolve an exact compatible version before requesting its approved image."""
        family = await self._releases.history(model_id)
        version = next(
            (item.version for item in family if item.version.version_id == version_id),
            None,
        )
        if version is None or version.preview_url is None:
            return ()
        url = version.preview_url
        task = self._pending.get(url)
        if task is None:
            task = asyncio.create_task(self._prepare(url))
            self._pending[url] = task
            task.add_done_callback(lambda completed: self._release(url, completed))
        return await asyncio.shield(task)

    async def _prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Retain cached reads and repair through worker completion before allowing maintenance."""
        async with self._activity.work():
            stored = self._prepared.get(url)
            if stored is not None:
                if await complete_blocking_model_work(self._previews.available, stored):
                    return stored
                self._prepared.pop(url, None)
            images = await self._previews.prepare(url)
            self._prepared[url] = images
            return images

    def _release(
        self, url: str, task: asyncio.Task[tuple[ThumbnailReference, ...]]
    ) -> None:
        """Release finished work and observe failures even after every consumer dismisses."""
        if self._pending.get(url) is task:
            self._pending.pop(url)
        if not task.cancelled():
            error = task.exception()
            if error is not None:
                _LOGGER.warning(
                    "Model version preview preparation failed; a later request may retry",
                    exc_info=(type(error), error, error.__traceback__),
                )

    async def close(self) -> None:
        """Cancel independent preview work when the owning Comfy host exits."""
        tasks = tuple(self._pending.values())
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        self._pending.clear()
