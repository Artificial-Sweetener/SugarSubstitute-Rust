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
"""Own shared non-secret provider preferences through one transactional repository."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import replace
from typing import Protocol

from ..domain.provider_preferences import (
    ModelProviderPreferences,
    ModelProviderPreferencePatch,
)
from ..domain.thumbnail_safety import CivitaiThumbnailSafety


class ModelProviderPreferenceRepository(Protocol):
    """Persist immutable settings while merging concurrent consumer changes atomically."""

    def load(self) -> ModelProviderPreferences:
        """Read the authoritative persisted settings."""

    def change(
        self, update: Callable[[ModelProviderPreferences], ModelProviderPreferences]
    ) -> ModelProviderPreferences:
        """Commit one settings transition against the latest persisted snapshot."""


class ModelProviderPreferenceService:
    """Keep settings policy shared across all live adapters and frontends."""

    def __init__(self, repository: ModelProviderPreferenceRepository) -> None:
        """Bind the configured model-root repository."""
        self._repository = repository

    def read(self) -> ModelProviderPreferences:
        """Read current preferences without starting any provider request."""
        return self._repository.load()

    def configure(
        self, patch: ModelProviderPreferencePatch
    ) -> ModelProviderPreferences:
        """Merge only explicit choices, preserving changes from other consumers."""
        return self._repository.change(
            lambda current: replace(
                current,
                metadata_lookup_enabled=patch.get(
                    "metadata_lookup_enabled", current.metadata_lookup_enabled
                ),
                missing_model_lookup_enabled=patch.get(
                    "missing_model_lookup_enabled", current.missing_model_lookup_enabled
                ),
                downloads_enabled=patch.get(
                    "downloads_enabled", current.downloads_enabled
                ),
                thumbnail_downloads_enabled=patch.get(
                    "thumbnail_downloads_enabled", current.thumbnail_downloads_enabled
                ),
                thumbnail_safety=patch.get(
                    "thumbnail_safety", current.thumbnail_safety
                ),
            )
        )

    def image_policy(self) -> CivitaiThumbnailSafety:
        """Apply current thumbnail settings at each provider selection boundary."""
        return self.read().image_policy()

    def metadata_lookup_enabled(self) -> bool:
        """Allow local CivitAI hash queries only under current shared policy."""
        return self.read().metadata_lookup_enabled

    def missing_model_lookup_enabled(self) -> bool:
        """Apply explicit recipe hash lookup independently of automatic enrichment."""
        return self.read().missing_model_lookup_enabled

    def downloads_enabled(self) -> bool:
        """Read missing-model CivitAI acquisition consent independently of lookup policy."""
        return self.read().downloads_enabled

    def require_missing_model_downloads(self) -> None:
        """Reject recipe acquisition opt-out before hash lookup or download allocation."""
        if not self.downloads_enabled():
            raise ValueError("CivitAI downloads are disabled.")

    def enrichment_enabled(self, kind: str) -> bool:
        """Keep OpenModelDB enrichment available independently of CivitAI lookup policy."""
        return kind == "upscale_models" or self.metadata_lookup_enabled()

    def enrichment_policy(self, kind: str) -> str:
        """Version thumbnail decisions independently for OpenModelDB and configured CivitAI policy."""
        return (
            f"openmodeldb-thumbnail-v1|civitai-thumbnail-v2:{self.image_policy().value}"
            if kind == "upscale_models"
            else f"civitai-thumbnail-v2:{self.image_policy().value}"
        )

    def enrichment_provider_scope(self, kind: str) -> str:
        """Invalidate upscaler misses when CivitAI lookup becomes eligible alongside OpenModelDB."""
        return ",".join(self.enrichment_providers(kind))

    def enrichment_providers(self, kind: str) -> tuple[str, ...]:
        """Name eligible providers explicitly for durable maintenance and retry ownership."""
        if kind == "upscale_models":
            return (
                ("openmodeldb", "civitai")
                if self.metadata_lookup_enabled()
                else ("openmodeldb",)
            )
        return ("civitai",)
