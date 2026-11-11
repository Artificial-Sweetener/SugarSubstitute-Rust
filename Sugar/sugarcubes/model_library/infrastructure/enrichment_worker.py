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
"""Schedule serial, coalesced enrichment independently of scans and foreground requests."""

from __future__ import annotations

import asyncio
from collections import OrderedDict
from collections.abc import Sequence, AsyncIterator
from contextlib import asynccontextmanager
import logging

from ..application.automatic_enrichment import AutomaticModelEnrichment
from ..application.ports import ModelCatalogRepository
from .catalog_updates import ModelCatalogUpdates
from .jobs import ModelBackgroundJobs, ModelJob

_LOGGER = logging.getLogger(__name__)


class ModelEnrichmentWorker:
    """Process one model at a time and prioritize model fields currently in use."""

    def __init__(
        self,
        repository: ModelCatalogRepository,
        enrichment: AutomaticModelEnrichment,
        jobs: ModelBackgroundJobs,
        updates: ModelCatalogUpdates,
        interval: float = 60,
    ) -> None:
        """Bind authoritative use-case, job and publication owners to a bounded scheduler."""
        self._repository = repository
        self._enrichment = enrichment
        self._jobs = jobs
        self._updates = updates
        self._interval = interval
        self._pending: OrderedDict[str, bool] = OrderedDict()
        self._wake = asyncio.Event()
        self._task: asyncio.Task[None] | None = None
        self._current: str | None = None
        self._paused = 0
        self._generation = 0
        self._ready = asyncio.Event()
        self._ready.set()
        self._idle = asyncio.Event()
        self._idle.set()

    def start(self) -> None:
        """Start once after scanning publishes approved catalog evidence."""
        if self._task is None:
            self._task = asyncio.create_task(self._run())
            self._task.add_done_callback(self._observe_failure)

    def prioritize(self, identities: Sequence[str]) -> None:
        """Move visible choices ahead of the remaining startup backlog without duplication."""
        for identity in reversed(identities):
            if identity != self._current:
                self._pending.setdefault(identity, True)
                self._pending.move_to_end(identity, last=False)
        self._wake.set()

    async def refill(self) -> None:
        """Append current candidates while retaining visible-model priority."""
        await self._ready.wait()
        generation = self._generation
        entries = await asyncio.to_thread(self._repository.list_entries)
        for entry in sorted(entries, key=lambda item: item.evidence.size_bytes):
            identity = entry.evidence.location_id
            if identity != self._current and await asyncio.to_thread(
                self._enrichment.needs_refresh, entry
            ):
                if generation == self._generation:
                    self._pending.setdefault(identity, True)
        self._wake.set()

    @asynccontextmanager
    async def pause(self) -> AsyncIterator[None]:
        """Drain the current automatic job and prevent a new scheduler decision during maintenance."""
        self._paused += 1
        self._ready.clear()
        try:
            await self._idle.wait()
            yield
        finally:
            self._paused -= 1
            if not self._paused:
                self._ready.set()
                self._wake.set()

    def discard_pending(self) -> None:
        """Invalidate both queued candidates and refill results chosen before maintenance."""
        self._generation += 1
        self._pending.clear()

    async def request_refresh(self, provider: str) -> None:
        """Queue deliberate current-policy refresh without deleting existing presentation."""
        entries = await asyncio.to_thread(self._repository.list_entries)
        for entry in entries:
            if self._enrichment.supports_provider(entry, provider):
                self._pending[entry.evidence.location_id] = False
        self._wake.set()
        self.start()

    async def refresh(self, model_id: str, *, automatic: bool = False) -> ModelJob:
        """Share one per-model job between automatic and explicit consumer requests."""
        if await asyncio.to_thread(self._repository.find_entry, model_id) is None:
            raise ValueError("The requested model was not found in the catalog.")

        async def operation() -> dict[str, object]:
            """Notify consumers after the application owner commits its enrichment result."""
            result = await self._enrichment.refresh(model_id, automatic=automatic)
            await self._updates.publish()
            return result

        return self._jobs.start(f"metadata-refresh:{model_id}", operation)

    def status(self) -> dict[str, object]:
        """Report progress independently of the completed stat-only scan."""
        return {
            "queued": len(self._pending),
            "modelId": self._current,
            "running": self._task is not None and not self._task.done(),
        }

    async def close(self) -> None:
        """Drain the scheduler before the shared job owner closes its operations."""
        if self._task is not None:
            self._task.cancel()
            await asyncio.gather(self._task, return_exceptions=True)
            self._task = None
        self._pending.clear()

    async def _run(self) -> None:
        """Drain serial batches and reconsider durable retry decisions on idle turns."""
        while True:
            self._wake.clear()
            while self._pending:
                await self._ready.wait()
                if not self._pending:
                    break
                self._current, automatic = self._pending.popitem(last=False)
                self._idle.clear()
                try:
                    job = await self.refresh(self._current, automatic=automatic)
                    await self._jobs.wait(job.job_id)
                except (OSError, RuntimeError, ValueError):
                    _LOGGER.exception(
                        "Automatic model enrichment could not be scheduled",
                        extra={"model_id": self._current},
                    )
                finally:
                    self._current = None
                    self._idle.set()
            try:
                await asyncio.wait_for(self._wake.wait(), timeout=self._interval)
            except TimeoutError:
                await self.refill()

    def _observe_failure(self, task: asyncio.Task[None]) -> None:
        """Surface unexpected lifecycle failures rather than silently abandoning the backlog."""
        if not task.cancelled():
            error = task.exception()
            if error is not None:
                _LOGGER.error(
                    "Automatic model enrichment worker stopped",
                    exc_info=(type(error), error, error.__traceback__),
                )
