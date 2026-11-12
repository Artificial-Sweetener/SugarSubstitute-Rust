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
"""Describe validated provider evidence independently of HTTP and browser presentation."""

from __future__ import annotations

from dataclasses import dataclass

from .models import ModelMetadata, ThumbnailReference
from .provider_links import ModelProviderLink


@dataclass(frozen=True, slots=True)
class ProviderModelVersion:
    """Carry normalized provider labels and one approved image candidate."""

    provider: str
    model_id: str
    version_id: str
    title: str
    version: str
    base_model: str
    page_url: str
    description: str
    trained_words: tuple[str, ...]
    preview_url: str | None
    provider_links: tuple[ModelProviderLink, ...] = ()
    model_type: str = ""
    tags: tuple[str, ...] = ()
    provider_nsfw: bool | None = None
    provider_nsfw_level: int | None = None

    def metadata(self, thumbnails: tuple[ThumbnailReference, ...]) -> ModelMetadata:
        """Project enriched labels while retaining exact Comfy filenames elsewhere."""
        return ModelMetadata(
            title=self.title,
            version=self.version,
            base_model=self.base_model,
            provider=self.provider,
            provider_url=self.page_url,
            description=self.description,
            trained_words=self.trained_words,
            thumbnails=thumbnails,
            provider_model_id=self.model_id,
            provider_version_id=self.version_id,
            provider_links=self.provider_links,
            tags=self.tags,
            model_type=self.model_type,
            provider_nsfw=self.provider_nsfw,
            provider_nsfw_level=self.provider_nsfw_level,
        )
