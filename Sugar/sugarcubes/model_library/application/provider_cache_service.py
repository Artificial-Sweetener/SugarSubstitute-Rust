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
"""Coordinate deliberate cache maintenance through the shared publication barrier."""

from __future__ import annotations

from collections.abc import Callable
from contextlib import AbstractAsyncContextManager
from typing import Protocol

from .blocking_model_work import complete_blocking_model_work
from .metadata_activity import ModelMetadataActivity
from ..domain.provider_cache import ProviderCacheAction, ProviderCacheSummary


class ProviderCacheStorage(Protocol):
    """Own provider-specific persistence and physical thumbnail collection."""

    def summary(self) -> ProviderCacheSummary:
        """Count authoritative persisted records and available prepared assets."""

    def clear_thumbnails(self) -> None:
        """Strip provider image references while retaining completed lookup decisions."""

    def clear_metadata(self) -> None:
        """Clear provider matches, misses and retries while preserving unrelated ownership."""

    def clear_retries(self) -> None:
        """Allow a deliberate refresh to retry prior failures without deleting presentation."""


class ProviderCacheQueue(Protocol):
    """Own scheduling while cache operations replace durable provider decisions."""

    def pause(self) -> AbstractAsyncContextManager[None]:
        """Drain automatic work and hold subsequent scheduling until maintenance finishes."""

    def discard_pending(self) -> None:
        """Discard candidates chosen before the maintenance transition."""

    async def request_refresh(self, provider: str) -> None:
        """Queue explicit refresh only for kinds where the provider is enabled."""


class ProviderCacheUpdates(Protocol):
    """Publish committed model presentation changes to every connected consumer."""

    async def invalidate(self) -> None:
        """Invalidate prepared client snapshots after durable maintenance completes."""


class ModelProviderCacheService:
    """Keep reference clear and refresh actions distinct across all consumers."""

    def __init__(
        self,
        storage: ProviderCacheStorage,
        activity: ModelMetadataActivity,
        queue: ProviderCacheQueue,
        updates: ProviderCacheUpdates,
        invalidate_missing: Callable[[], None],
        *,
        provider: str,
    ) -> None:
        """Bind one provider's storage, queue and process cache to the host publication lease."""
        self._storage = storage
        self._activity = activity
        self._queue = queue
        self._updates = updates
        self._invalidate_missing = invalidate_missing
        self._provider = provider

    async def summary(self) -> ProviderCacheSummary:
        """Read a coherent usage snapshot after prior publication drains."""
        async with self._activity.maintenance():
            return await complete_blocking_model_work(self._storage.summary)

    async def execute(
        self, action: ProviderCacheAction, commit: Callable[[], None]
    ) -> ProviderCacheSummary:
        """Finish committed maintenance before notifying clients or releasing queued refresh work."""
        async with self._queue.pause():
            async with self._activity.maintenance():
                commit()
                if action == "clear_thumbnails":
                    await complete_blocking_model_work(self._storage.clear_thumbnails)
                elif action == "clear_metadata":
                    await complete_blocking_model_work(self._storage.clear_metadata)
                    self._invalidate_missing()
                else:
                    await complete_blocking_model_work(self._storage.clear_retries)
                    self._invalidate_missing()
                self._queue.discard_pending()
                summary = await complete_blocking_model_work(self._storage.summary)
            await self._updates.invalidate()
            if action == "refresh":
                await self._queue.request_refresh(self._provider)
            return summary
