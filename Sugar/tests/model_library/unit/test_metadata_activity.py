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
"""Prove complete work leases, maintenance priority and cancellation recovery."""

from __future__ import annotations

import asyncio
import pytest

from sugarcubes.model_library.application.metadata_activity import ModelMetadataActivity


def test_queued_maintenance_waits_for_nested_work_and_precedes_late_readers() -> None:
    """Let existing operations finish nested calls while blocking new work behind maintenance."""

    async def exercise() -> None:
        """Observe ordered externally visible work and maintenance transitions."""
        activity = ModelMetadataActivity()
        started, release, entered, finish = (asyncio.Event() for _ in range(4))
        events: list[str] = []

        async def work() -> None:
            """Retain nested publication inside the existing operation's lease."""
            async with activity.work():
                started.set()
                await release.wait()
                async with activity.work():
                    events.append("nested")
                events.append("published")

        async def maintenance() -> None:
            """Hold one exclusive transition before allowing new work."""
            async with activity.maintenance():
                events.append("maintenance")
                entered.set()
                await finish.wait()

        async def late() -> None:
            """Record a request submitted after the maintenance barrier was queued."""
            async with activity.work():
                events.append("late")

        first = asyncio.create_task(work())
        await started.wait()
        writer = asyncio.create_task(maintenance())
        await asyncio.sleep(0)
        last = asyncio.create_task(late())
        await asyncio.sleep(0)
        assert not entered.is_set() and not last.done()
        release.set()
        await asyncio.wait_for(entered.wait(), 1)
        assert events == ["nested", "published", "maintenance"]
        assert not last.done()
        finish.set()
        await asyncio.gather(first, writer, last)
        assert events == ["nested", "published", "maintenance", "late"]

    asyncio.run(exercise())


def test_cancelled_waiting_maintenance_releases_priority_without_interrupting_readers() -> (
    None
):
    """Let pending reads proceed after a cancelled writer leaves the queue."""

    async def exercise() -> None:
        """Keep one active reader while cancelling the unrelated maintenance waiter."""
        activity = ModelMetadataActivity()
        started, release, last_entered = (asyncio.Event() for _ in range(3))

        async def work(signal: asyncio.Event) -> None:
            """Retain a caller until the proof releases its work."""
            async with activity.work():
                signal.set()
                await release.wait()

        async def maintenance() -> None:
            """Fail if an exclusive transition runs before the first reader finishes."""
            async with activity.maintenance():
                raise AssertionError("Maintenance should be cancelled while waiting.")

        first = asyncio.create_task(work(started))
        await started.wait()
        writer = asyncio.create_task(maintenance())
        await asyncio.sleep(0)
        last = asyncio.create_task(work(last_entered))
        await asyncio.sleep(0)
        assert not last_entered.is_set()
        writer.cancel()
        with pytest.raises(asyncio.CancelledError):
            await writer
        await asyncio.wait_for(last_entered.wait(), 1)
        assert not first.done()
        release.set()
        await asyncio.gather(first, last)
        async with activity.maintenance():
            pass

    asyncio.run(exercise())


def test_failed_maintenance_releases_lease_and_upgrade_is_rejected() -> None:
    """Keep failure and invalid nested maintenance from stranding future operations."""

    async def exercise() -> None:
        """Verify both failure paths against subsequent legitimate work."""
        activity = ModelMetadataActivity()
        with pytest.raises(ValueError, match="fixture"):
            async with activity.maintenance():
                raise ValueError("fixture maintenance failure")
        async with activity.work():
            with pytest.raises(RuntimeError, match="upgrade"):
                async with activity.maintenance():
                    pass
        async with activity.maintenance():
            pass

    asyncio.run(exercise())
