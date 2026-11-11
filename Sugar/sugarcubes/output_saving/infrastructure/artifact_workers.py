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
"""Admit two active artifact workers and two queued requests with observed cancellation and shutdown."""

import asyncio
from collections.abc import Callable

from ..domain.artifact import ArtifactReceipt, ArtifactUnavailable
from .artifact_budget import ArtifactBudget


class ArtifactWorkers:
    """Retain every worker's lifetime until its blocking file operation actually returns."""

    def __init__(self) -> None:
        """Create bounded admission without starting threads or tasks before a request arrives."""
        self._slots = asyncio.Semaphore(2)
        self._admitted = 0
        self._active: dict[asyncio.Task[ArtifactReceipt], ArtifactBudget] = {}
        self._closed = False
        self._idle = asyncio.Event()
        self._idle.set()

    async def run(
        self, operation: Callable[[ArtifactBudget], ArtifactReceipt]
    ) -> ArtifactReceipt:
        """Reject overload, offload hashing and retain the slot through actual cancelled-worker return."""
        if self._closed:
            raise ArtifactUnavailable(
                "cancelled", "Output inspection is shutting down."
            )
        if self._admitted >= 4:
            raise ArtifactUnavailable(
                "budget", "Output inspection is busy; retry the checkpoint."
            )
        self._admitted += 1
        self._idle.clear()
        try:
            async with self._slots:
                if self._closed:
                    raise ArtifactUnavailable(
                        "cancelled", "Output inspection is shutting down."
                    )
                budget = ArtifactBudget()
                worker = asyncio.create_task(asyncio.to_thread(operation, budget))
                self._active[worker] = budget
                try:
                    return await asyncio.shield(worker)
                except asyncio.CancelledError:
                    budget.cancelled.set()
                    await _observe(worker)
                    raise
                finally:
                    self._active.pop(worker, None)
        finally:
            self._admitted -= 1
            if self._admitted == 0:
                self._idle.set()

    async def close(self) -> None:
        """Refuse new work, cancel active I/O cooperatively and observe all admitted terminal outcomes."""
        self._closed = True
        for budget in self._active.values():
            budget.cancelled.set()
        await self._idle.wait()


async def _observe(worker: asyncio.Task[ArtifactReceipt]) -> None:
    """Keep a cancelled request's resources owned even when cancellation is repeated during retirement."""
    while not worker.done():
        try:
            await asyncio.shield(worker)
        except asyncio.CancelledError:
            continue
        except ArtifactUnavailable:
            return
    if not worker.cancelled():
        # Retrieve even an unavailable outcome so its exception never becomes an unobserved task.
        try:
            worker.result()
        except ArtifactUnavailable:
            return
