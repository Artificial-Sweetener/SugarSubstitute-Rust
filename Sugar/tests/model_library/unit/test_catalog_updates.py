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
"""Prove one owner announces committed catalog revisions, at once and then at a bounded pace."""

from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from sugarcubes.model_library.infrastructure.catalog_store import (
    SqliteModelCatalogRepository,
)
from sugarcubes.model_library.infrastructure.catalog_updates import ModelCatalogUpdates
from tests.support.no_progress import await_outcome


class _Pace:
    """Stand in for the monotonic clock and for the pause between announcements."""

    def __init__(self) -> None:
        """Start at a fixed instant with no pause taken."""
        self.now = 100.0
        self.pauses: list[float] = []

    def clock(self) -> float:
        """Report the controlled instant."""
        return self.now

    async def pause(self, seconds: float) -> None:
        """Record a requested pause and move the clock past it without waiting."""
        self.pauses.append(seconds)
        self.now += seconds


def _updates(
    tmp_path: Path,
) -> tuple[SqliteModelCatalogRepository, ModelCatalogUpdates, _Pace]:
    repository = SqliteModelCatalogRepository(tmp_path)
    pace = _Pace()
    return (
        repository,
        ModelCatalogUpdates(repository, clock=pace.clock, pause=pace.pause),
        pace,
    )


def test_first_change_after_quiet_is_announced_at_once(tmp_path: Path) -> None:
    """Answer a consumer that is behind without any pause."""
    repository, updates, pace = _updates(tmp_path)

    async def exercise() -> None:
        await updates.invalidate()
        first = await updates.wait(0)
        assert first == 1
        pace.now += 60
        await updates.invalidate()
        assert await updates.wait(first) == 2

    asyncio.run(exercise())
    assert pace.pauses == []
    assert repository.revision() == 2


def test_continuing_changes_reach_a_consumer_once_per_interval_with_the_newest_revision(
    tmp_path: Path,
) -> None:
    """Collapse an ingest's many commits into one announcement per interval."""
    _repository, updates, pace = _updates(tmp_path)

    async def exercise() -> None:
        await updates.invalidate()
        first = await updates.wait(0)
        assert first == 1
        for _ in range(25):
            await updates.invalidate()
        pace.now += 0.1
        second = await updates.wait(first)
        assert second == 26
        for _ in range(4):
            await updates.invalidate()
        assert await updates.wait(second) == 30

    asyncio.run(exercise())
    assert pace.pauses == [pytest.approx(0.15), pytest.approx(0.25)]


def test_a_consumer_behind_the_last_announcement_is_not_paced(tmp_path: Path) -> None:
    """Let a second consumer catch up while the first is held to the interval."""
    _repository, updates, pace = _updates(tmp_path)

    async def exercise() -> None:
        await updates.invalidate()
        held = await updates.wait(0)
        await updates.invalidate()
        assert held == 1
        pace.now += 60
        assert await updates.wait(held) == 2
        assert await updates.wait(0) == 2

    asyncio.run(exercise())
    assert pace.pauses == []


def test_a_commit_on_another_thread_wakes_a_waiting_consumer(tmp_path: Path) -> None:
    """Announce a store's own commit without any caller publishing it."""
    repository, updates, _pace = _updates(tmp_path)

    async def exercise() -> int:
        waiting = asyncio.create_task(updates.wait(0))
        await asyncio.sleep(0)
        await asyncio.sleep(0)
        await asyncio.to_thread(repository.database.invalidate)
        return await await_outcome(waiting)

    assert asyncio.run(exercise()) == 1


def test_negative_revisions_are_rejected(tmp_path: Path) -> None:
    """Refuse a revision no catalog can hold."""
    _repository, updates, _pace = _updates(tmp_path)
    with pytest.raises(ValueError, match="nonnegative"):
        asyncio.run(updates.wait(-1))
