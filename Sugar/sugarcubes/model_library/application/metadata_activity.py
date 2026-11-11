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
"""Serialize maintenance with complete metadata operations and their physical publication."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator, Callable
from contextlib import asynccontextmanager


class ModelMetadataActivity:
    """Give queued maintenance priority while allowing same-task nested metadata work."""

    def __init__(self) -> None:
        """Own host-loop leases explicitly; independently scheduled tasks receive separate leases."""
        self._readers: dict[asyncio.Task[object], int] = {}
        self._writer = False
        self._waiting_writers = 0
        self._changed = asyncio.Event()

    @asynccontextmanager
    async def work(self) -> AsyncIterator[None]:
        """Retain an operation through nested provider, preview and persistence calls."""
        task = asyncio.current_task()
        if task is None:
            raise RuntimeError("Metadata work requires an active task.")
        if task not in self._readers:
            await self._wait(lambda: not self._writer and self._waiting_writers == 0)
        self._readers[task] = self._readers.get(task, 0) + 1
        try:
            yield
        finally:
            remaining = self._readers[task] - 1
            if remaining:
                self._readers[task] = remaining
            else:
                del self._readers[task]
            self._changed.set()

    @asynccontextmanager
    async def maintenance(self) -> AsyncIterator[None]:
        """Wait for complete publication, then exclude new metadata work until the transition ends."""
        if asyncio.current_task() in self._readers:
            raise RuntimeError("Metadata work cannot upgrade its lease to maintenance.")
        self._waiting_writers += 1
        try:
            await self._wait(lambda: not self._writer and not self._readers)
            self._writer = True
        finally:
            self._waiting_writers -= 1
            self._changed.set()
        try:
            yield
        finally:
            self._writer = False
            self._changed.set()

    async def _wait(self, ready: Callable[[], bool]) -> None:
        """Wait on state changes without holding a lock or interrupting lease cleanup."""
        while not ready():
            await self._changed.wait()
            self._changed.clear()
