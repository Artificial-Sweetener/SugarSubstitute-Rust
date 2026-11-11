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
"""Publish cheap catalog invalidation tokens to browser and desktop consumers."""

from __future__ import annotations

import asyncio

from .catalog_store import SqliteModelCatalogRepository
from ..application.blocking_model_work import complete_blocking_model_work


class ModelCatalogUpdates:
    """Wake catalog consumers after committed library mutations without resending image data."""

    def __init__(self, repository: SqliteModelCatalogRepository) -> None:
        """Bind the persisted revision as the authoritative change token."""
        self._repository = repository
        self._condition = asyncio.Condition()

    async def publish(self) -> None:
        """Notify only after the mutation owner has committed its catalog work."""
        async with self._condition:
            self._condition.notify_all()

    async def invalidate(self) -> None:
        """Advance the durable token before waking every catalog consumer for deliberate cache actions."""
        await complete_blocking_model_work(self._repository.database.invalidate)
        await self.publish()

    async def wait(self, after_revision: int) -> int:
        """Return immediately for stale clients or wait at most fifteen seconds for change."""
        if after_revision < 0:
            raise ValueError("Model catalog revision must be nonnegative.")
        async with self._condition:
            revision = await asyncio.to_thread(self._repository.revision)
            if revision != after_revision:
                return revision
            try:
                await asyncio.wait_for(self._condition.wait(), timeout=15)
            except TimeoutError:
                pass
            return await asyncio.to_thread(self._repository.revision)
