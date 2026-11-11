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
"""Own successful daily release freshness and shared provider requests."""

from __future__ import annotations

import asyncio
import logging
from collections.abc import Callable
from dataclasses import dataclass
from datetime import datetime
from typing import Protocol

from ..domain.provider_releases import ModelProviderRelease
from .metadata_activity import ModelMetadataActivity

_LOGGER = logging.getLogger(__name__)


class ModelReleaseProvider(Protocol):
    """Resolve normalized provider history without exposing transport mechanics."""

    async def releases(self, model_id: str) -> tuple[ModelProviderRelease, ...]:
        """Return normalized releases for a verified provider identity."""


@dataclass(frozen=True, slots=True)
class ReleaseHistorySnapshot:
    """Retain the last successful response independently of refresh failures."""

    checked_at: datetime
    releases: tuple[ModelProviderRelease, ...]


class ReleaseHistoryRepository(Protocol):
    """Persist validated history separately from workflow usage and consent."""

    def read(self, key: str) -> ReleaseHistorySnapshot | None:
        """Return the last successful page response."""

    def save(self, key: str, snapshot: ReleaseHistorySnapshot) -> None:
        """Replace history only after a successful provider response."""


class MemoryReleaseHistory:
    """Supply process-scoped storage for hosts without durable history storage."""

    def __init__(self) -> None:
        """Keep immutable page snapshots in one local collection."""
        self._snapshots: dict[str, ReleaseHistorySnapshot] = {}

    def read(self, key: str) -> ReleaseHistorySnapshot | None:
        """Read an immutable successful response."""
        return self._snapshots.get(key)

    def save(self, key: str, snapshot: ReleaseHistorySnapshot) -> None:
        """Publish the complete successful response atomically."""
        self._snapshots[key] = snapshot


class ModelReleaseHistory:
    """Share daily freshness across installed versions, browsers and picker instances."""

    def __init__(
        self,
        provider: ModelReleaseProvider,
        repository: ReleaseHistoryRepository,
        *,
        clock: Callable[[], datetime] = lambda: datetime.now().astimezone(),
        scope: Callable[[], str] = lambda: "",
        activity: ModelMetadataActivity | None = None,
    ) -> None:
        """Use host calendar days and isolate history when provider visibility policy changes."""
        self.provider = provider
        self.repository = repository
        self.clock = clock
        self.scope = scope
        self._activity = activity or ModelMetadataActivity()
        self._pending: dict[str, asyncio.Task[tuple[ModelProviderRelease, ...]]] = {}

    def cached(self, model_id: str) -> ReleaseHistorySnapshot | None:
        """Read the current policy's successful history without refreshing or provider traffic."""
        return self.repository.read(f"{self.scope()}:{model_id}")

    async def releases(
        self, model_id: str, *, refresh: bool = False
    ) -> tuple[ModelProviderRelease, ...]:
        """Refresh once per host day, or explicitly when browsing versions or acquiring a file."""
        now = self.clock()
        if now.tzinfo is None:
            raise ValueError("Release check timestamps must be timezone-aware.")
        key = f"{self.scope()}:{model_id}"
        pending = self._pending.get(key)
        if pending is None:
            snapshot = self.repository.read(key)
            if (
                not refresh
                and snapshot is not None
                and snapshot.checked_at.astimezone(now.tzinfo).date() == now.date()
            ):
                return snapshot.releases
            pending = asyncio.create_task(self._refresh(key, model_id))
            self._pending[key] = pending
            pending.add_done_callback(lambda task: self._finished(key, task))
        return await asyncio.shield(pending)

    async def _refresh(
        self, key: str, model_id: str
    ) -> tuple[ModelProviderRelease, ...]:
        """Commit freshness after network validation succeeds, preserving the previous success on error."""
        async with self._activity.work():
            releases = await self.provider.releases(model_id)
            self.repository.save(key, ReleaseHistorySnapshot(self.clock(), releases))
            return releases

    def _finished(
        self, key: str, task: asyncio.Task[tuple[ModelProviderRelease, ...]]
    ) -> None:
        """Release shared work and observe errors even when a caller has disconnected."""
        if self._pending.get(key) is task:
            self._pending.pop(key)
        if not task.cancelled() and (error := task.exception()) is not None:
            _LOGGER.warning(
                "Model release refresh failed; retaining previous history",
                extra={"provider_model_id": key},
                exc_info=(type(error), error, error.__traceback__),
            )

    async def close(self) -> None:
        """Cancel owned requests when the Comfy host shuts down."""
        tasks = tuple(self._pending.values())
        for task in tasks:
            task.cancel()
        await asyncio.gather(*tasks, return_exceptions=True)
        self._pending.clear()
