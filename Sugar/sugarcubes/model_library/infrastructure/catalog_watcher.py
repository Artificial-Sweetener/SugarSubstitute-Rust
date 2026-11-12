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
"""Publish stable filesystem changes through the shared catalog refresh owner."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
import logging

from ..application.ports import ModelCatalogRepository
from .folder_observations import ModelFolderObservations
from .jobs import ModelBackgroundJobs, ModelJob

_LOGGER = logging.getLogger(__name__)


class ModelCatalogWatcher:
    """Own one host-scoped polling task with no independent catalog or job state."""

    def __init__(
        self,
        observations: ModelFolderObservations,
        repository: ModelCatalogRepository,
        jobs: ModelBackgroundJobs,
        refresh: Callable[[tuple[str, ...]], ModelJob],
        interval: float = 5,
    ) -> None:
        """Compose observation and existing refresh policies without duplicating either."""
        self._observations = observations
        self._repository = repository
        self._jobs = jobs
        self._refresh = refresh
        self._interval = interval
        self._task: asyncio.Task[None] | None = None

    def start(self, initial: ModelJob) -> None:
        """Start once after the existing cold scan has established the shared catalog."""
        if self._task is None:
            self._task = asyncio.create_task(self._run(initial))
            self._task.add_done_callback(self._observe_failure)

    async def close(self) -> None:
        """Drain the observer before closing shared jobs during host shutdown."""
        if self._task is not None:
            self._task.cancel()
            await asyncio.gather(self._task, return_exceptions=True)
            self._task = None

    async def check_once(self) -> None:
        """Refresh stable dirty kinds and retain them for retry if the shared job fails."""
        kinds = await asyncio.to_thread(self._observations.poll)
        if not kinds:
            return
        job = self._refresh(kinds)
        await self._jobs.wait(job.job_id)
        if job.status != "completed":
            self._observations.retry(kinds)
            raise RuntimeError(job.error or "The model folder refresh failed.")

    async def _run(self, initial: ModelJob) -> None:
        """Keep failures logged and retryable while honoring host task cancellation."""
        await self._jobs.wait(initial.job_id)
        entries = await asyncio.to_thread(self._repository.list_entries)
        await asyncio.to_thread(
            self._observations.establish, tuple(entry.evidence for entry in entries)
        )
        if initial.status != "completed":
            self._observations.retry_all()
        while True:
            await asyncio.sleep(self._interval)
            try:
                await self.check_once()
            except (OSError, RuntimeError, ValueError):
                _LOGGER.exception(
                    "Shared model folder refresh failed; the next observation will retry"
                )

    def _observe_failure(self, task: asyncio.Task[None]) -> None:
        """Surface unexpected lifecycle failures instead of abandoning a silent observer."""
        if not task.cancelled():
            error = task.exception()
            if error is not None:
                _LOGGER.error(
                    "Shared model catalog watcher stopped",
                    exc_info=(type(error), error, error.__traceback__),
                )
