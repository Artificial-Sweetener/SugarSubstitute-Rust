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
"""Retain a metadata activity lease through complete provider preview publication."""

from __future__ import annotations

from .enrichment_service import ModelPreviewPreparer
from .metadata_activity import ModelMetadataActivity
from ..domain.models import ThumbnailReference


class CoordinatedModelPreviews:
    """Share maintenance coordination across installed, version and discovery previews."""

    def __init__(
        self, activity: ModelMetadataActivity, previews: ModelPreviewPreparer
    ) -> None:
        """Bind lifecycle coordination without reimplementing preview or image selection policy."""
        self._activity = activity
        self._previews = previews

    def available(self, thumbnails: tuple[ThumbnailReference, ...]) -> bool:
        """Delegate prepared-file availability within the caller's retained activity lease."""
        return self._previews.available(thumbnails)

    async def prepare(self, url: str) -> tuple[ThumbnailReference, ...]:
        """Keep maintenance waiting until transport and physical preparation finish."""
        async with self._activity.work():
            return await self._previews.prepare(url)
