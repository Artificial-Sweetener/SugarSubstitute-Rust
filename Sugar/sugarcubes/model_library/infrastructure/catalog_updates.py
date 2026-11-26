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
"""Announce committed catalog revisions to browser and desktop consumers at a bounded pace."""

from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable, Sequence
import time

from .catalog_store import SqliteModelCatalogRepository
from ..application.blocking_model_work import complete_blocking_model_work


class ModelCatalogUpdates:
    """Own when consumers learn of a committed revision; mutation owners only commit.

    The first change after a quiet period is announced at once. While changes keep arriving,
    a consumer that already holds the last announced revision hears of the newest one at most
    once per interval, so a burst of commits reaches each consumer as one change. The interval
    is short, because a consumer is told which models changed and reads only those. The newest
    revision is always announced.
    """

    def __init__(
        self,
        repository: SqliteModelCatalogRepository,
        *,
        interval: float = 0.25,
        clock: Callable[[], float] = time.monotonic,
        pause: Callable[[float], Awaitable[None]] = asyncio.sleep,
    ) -> None:
        """Bind the persisted revision as the authoritative change token."""
        self._repository = repository
        self._interval = interval
        self._clock = clock
        self._pause = pause
        self._loop: asyncio.AbstractEventLoop | None = None
        self._waiters: list[asyncio.Future[None]] = []
        self._announced: tuple[int, float] | None = None
        repository.database.watch_commits(self._committed)

    async def invalidate(self, changed: Sequence[str] | None = None) -> None:
        """Announce a change no catalog row holds, for the named models or for all of them."""
        await complete_blocking_model_work(
            self._repository.database.invalidate, changed
        )

    async def wait(self, after_revision: int) -> int:
        """Return a newer revision, or the same one after fifteen quiet seconds."""
        if after_revision < 0:
            raise ValueError("Model catalog revision must be nonnegative.")
        loop = asyncio.get_running_loop()
        self._loop = loop
        # Registered before the read, so a commit between the read and the wait still wakes it.
        waiter: asyncio.Future[None] = loop.create_future()
        self._waiters.append(waiter)
        try:
            revision = await asyncio.to_thread(self._repository.revision)
            if revision == after_revision:
                try:
                    await asyncio.wait_for(waiter, timeout=15)
                except TimeoutError:
                    pass
                revision = await asyncio.to_thread(self._repository.revision)
                if revision == after_revision:
                    return revision
        finally:
            if waiter in self._waiters:
                self._waiters.remove(waiter)
        return await self._announce(after_revision, revision)

    async def _announce(self, held: int, revision: int) -> int:
        """Answer a consumer behind the last announcement at once and pace one that holds it."""
        announced = self._announced
        if announced is not None and held == announced[0]:
            delay = announced[1] + self._interval - self._clock()
            if delay > 0:
                await self._pause(delay)
                revision = await asyncio.to_thread(self._repository.revision)
        self._announced = (revision, self._clock())
        return revision

    def _committed(self) -> None:
        """Hand a commit made on any thread to the host loop that owns the waiters."""
        loop = self._loop
        if loop is not None and not loop.is_closed():
            loop.call_soon_threadsafe(self._wake)

    def _wake(self) -> None:
        """Release every consumer waiting for a change."""
        waiters, self._waiters = self._waiters, []
        for waiter in waiters:
            if not waiter.done():
                waiter.set_result(None)
