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
"""Apply provider cache persistence without claiming local or user-owned presentation."""

from __future__ import annotations

from ..domain.provider_cache import ProviderCacheSummary
from .provider_metadata_store import SqliteModelProviderMetadata
from .enrichment_cache import SqliteModelEnrichmentCache
from .enrichment_attempts import SqliteEnrichmentAttempts
from .thumbnail_assets import ModelThumbnailAssets
from .stored_thumbnail_ownership import StoredThumbnailOwnership
from .release_history_store import SqliteReleaseHistory


class CivitaiCacheStorage:
    """Maintain CivitAI evidence independently of every other provider and image origin."""

    def __init__(
        self,
        metadata: SqliteModelProviderMetadata,
        decisions: SqliteModelEnrichmentCache,
        attempts: SqliteEnrichmentAttempts,
        assets: ModelThumbnailAssets,
        ownership: StoredThumbnailOwnership,
        history: SqliteReleaseHistory,
    ) -> None:
        """Bind existing durable owners; callers retain the exclusive metadata activity lease."""
        self._metadata = metadata
        self._decisions = decisions
        self._attempts = attempts
        self._assets = assets
        self._ownership = ownership
        self._history = history

    def summary(self) -> ProviderCacheSummary:
        """Report distinct CivitAI records, source images, physical variants and bytes."""
        images = self._assets.source_summary("civitai")
        return ProviderCacheSummary(
            self._metadata.count("civitai"),
            images.source_count,
            images.variant_count,
            images.byte_size,
        )

    def clear_thumbnails(self) -> None:
        """Keep lookup decisions fresh and protect shared user/local images before collecting assets."""
        self._decisions.clear_thumbnails("civitai")
        self._metadata.clear_thumbnails("civitai")
        self._collect_thumbnails()

    def clear_metadata(self) -> None:
        """Invalidate positive and negative CivitAI decisions before releasing their images."""
        self._decisions.clear_provider("civitai")
        self._decisions.clear_thumbnails("civitai")
        self._attempts.clear_provider("civitai")
        self._metadata.clear_metadata("civitai")
        self._history.clear()
        self._collect_thumbnails()

    def clear_retries(self) -> None:
        """Reset failed and cancelled CivitAI work while retaining current labels and previews."""
        self._attempts.clear_provider("civitai")

    def _collect_thumbnails(self) -> None:
        """Backfill protected ownership before deleting provider source associations and orphan bytes."""
        self._assets.retain_sources(self._ownership.read())
        self._assets.clear_sources("civitai")
