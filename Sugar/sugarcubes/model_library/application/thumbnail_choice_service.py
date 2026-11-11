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
"""Own explicit preview replacement without modifying model files or provider evidence."""

from __future__ import annotations

from typing import Protocol

from .ports import ModelCatalogRepository
from ..domain.models import ThumbnailReference


class PreviewPreparation(Protocol):
    """Prepare bounded portable image variants through the infrastructure boundary."""

    def prepare(self, content: bytes) -> tuple[ThumbnailReference, ...]:
        """Validate image bytes and publish immutable variants."""


class ModelThumbnailChoiceService:
    """Keep user previews independent of automatic enrichment ownership."""

    def __init__(
        self, repository: ModelCatalogRepository, previews: PreviewPreparation
    ) -> None:
        """Bind the authoritative catalog and the shared preview preparer."""
        self._repository = repository
        self._previews = previews

    def replace(self, model_id: str, content: bytes) -> None:
        """Prepare an image for a known model and reject stale file evidence at commit."""
        entry = self._repository.find_entry(model_id)
        if entry is None:
            raise ValueError("The requested model was not found.")
        thumbnails = self._previews.prepare(content)
        if not self._repository.save_thumbnail_choice(entry.evidence, thumbnails):
            raise ValueError("The model changed before its preview could be saved.")
