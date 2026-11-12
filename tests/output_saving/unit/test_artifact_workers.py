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
"""Exercise finite artifact admission and observed cooperative cancellation without arbitrary sleeps."""

import asyncio
from threading import Event

import pytest

from sugarcubes.output_saving.domain.artifact import (
    ArtifactReceipt,
    ArtifactUnavailable,
)
from sugarcubes.output_saving.domain.transfer import OutputFileReference
from sugarcubes.output_saving.infrastructure.artifact_budget import ArtifactBudget
from sugarcubes.output_saving.infrastructure.artifact_workers import ArtifactWorkers
from tests.support.no_progress import wait_for_event


def test_two_active_two_queued_overload_and_observed_shutdown() -> None:
    """Retain blocking lifetimes until their actual outcomes while queued requests are refused on close."""

    async def exercise() -> None:
        """Synchronize real worker entry/release through explicit events rather than timing."""
        loop = asyncio.get_running_loop()
        entered: asyncio.Queue[int] = asyncio.Queue()
        release = Event()
        owner = ArtifactWorkers()
        budgets: list[ArtifactBudget] = []

        def operation(budget: ArtifactBudget) -> ArtifactReceipt:
            """Block only this test boundary until explicit release and then observe cancellation."""
            budgets.append(budget)
            loop.call_soon_threadsafe(entered.put_nowait, len(budgets))
            wait_for_event(release, "Test worker release was not delivered.")
            budget.check()
            return ArtifactReceipt(
                OutputFileReference("image.png", "", "output"),
                "0" * 64,
                10,
                1,
                1,
                "image/png",
                False,
            )

        active = [asyncio.create_task(owner.run(operation)) for _ in range(2)]
        await entered.get()
        await entered.get()
        queued = [asyncio.create_task(owner.run(operation)) for _ in range(2)]
        # Each marker runs after its preceding queued task reaches the finite slot wait.
        marker = loop.create_future()
        loop.call_soon(marker.set_result, None)
        await marker
        with pytest.raises(ArtifactUnavailable) as failure:
            await owner.run(operation)
        assert failure.value.code == "budget"
        active[0].cancel()
        closing = asyncio.create_task(owner.close())
        marker = loop.create_future()
        loop.call_soon(marker.set_result, None)
        await marker
        assert not closing.done()
        assert not active[0].done()
        assert len(budgets) == 2
        release.set()
        outcomes = await asyncio.gather(*active, *queued, return_exceptions=True)
        await closing
        assert isinstance(outcomes[0], asyncio.CancelledError)
        assert all(isinstance(value, ArtifactUnavailable) for value in outcomes[1:])
        assert all(budget.cancelled.is_set() for budget in budgets)
        with pytest.raises(ArtifactUnavailable) as failure:
            await owner.run(operation)
        assert failure.value.code == "cancelled"

    asyncio.run(exercise())


def test_artifact_budget_checks_exact_deadline_and_cumulative_metadata() -> None:
    """Verify boundary arithmetic deterministically without depending on a wall-clock delay."""
    now = [10.0]
    budget = ArtifactBudget(clock=lambda: now[0], seconds=15, maximum_metadata=4)
    now[0] = 24.999
    budget.check()
    budget.metadata(2)
    budget.metadata(2)
    with pytest.raises(ArtifactUnavailable) as failure:
        budget.metadata(1)
    assert failure.value.code == "budget"
    now[0] = 25
    with pytest.raises(ArtifactUnavailable) as failure:
        budget.check()
    assert failure.value.code == "budget"
