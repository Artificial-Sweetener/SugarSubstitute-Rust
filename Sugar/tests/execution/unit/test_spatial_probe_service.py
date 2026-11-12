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
"""Verify explicit probe execution, bounded handles and disconnected-caller ownership."""

from __future__ import annotations

import asyncio

import pytest

from sugarcubes.execution.models import ApiPrompt
from sugarcubes.execution.spatial_probe_graph import SpatialProbeGraph
from sugarcubes.execution.spatial_probe_service import SpatialProbeService
from sugarcubes.execution.spatial_probe_values import (
    SpatialFieldTarget,
    SpatialNodeContract,
    SpatialProbeExecution,
)


def contract(name: str) -> SpatialNodeContract:
    """Describe only external fixture schemas, keeping the production graph planner active."""
    return {
        "Source": SpatialNodeContract({}, ("IMAGE",)),
        "Sink": SpatialNodeContract({"image": "IMAGE"}, (), output_node=True),
    }[name]


def prompt() -> ApiPrompt:
    """Provide one output depending on a native width/height pair."""
    return {
        "source": {"class_type": "Source", "inputs": {"width": 512, "height": 512}},
        "sink": {"class_type": "Sink", "inputs": {"image": ["source", 0]}},
    }


class Execution:
    """Hold native work until its completion is explicitly observed by the test."""

    def __init__(self) -> None:
        """Retain calls and deterministic task boundaries."""
        self.calls: list[SpatialProbeExecution] = []
        self.started = asyncio.Event()
        self.finish = asyncio.Event()

    async def run(
        self, execution: SpatialProbeExecution
    ) -> tuple[tuple[int, int], tuple[int, int]]:
        """Wait at the external execution boundary and report a measured factor of two."""
        self.calls.append(execution)
        self.started.set()
        await self.finish.wait()
        return ((256, 128), (384, 256))


def test_prepare_never_executes_and_handles_are_bounded_and_expire() -> None:
    """Limit captured graphs without conflating inspection with generation execution."""
    clock = [0.0]
    execution = Execution()
    service = SpatialProbeService(
        SpatialProbeGraph(contract), execution, lambda: clock[0]
    )
    targets = (SpatialFieldTarget("source", "width"),), (
        SpatialFieldTarget("source", "height"),
    )
    token, plan = service.prepare(prompt(), *targets)
    assert [output.id for output in plan.outputs] == ["sink/image"]
    assert execution.calls == []
    for _ in range(3):
        service.prepare(prompt(), *targets)
    with pytest.raises(ValueError, match="earlier"):
        service.prepare(prompt(), *targets)
    clock[0] = 601
    with pytest.raises(ValueError, match="expired"):
        asyncio.run(service.measure(token, "sink/image"))
    service.prepare(prompt(), *targets)
    assert execution.calls == []


def test_measurement_is_deduplicated_and_finishes_after_caller_disconnects() -> None:
    """Keep one accepted job alive for owned cleanup while cancellation releases its caller."""

    async def run() -> None:
        """Cancel one of two callers and await the same measured result through the remaining one."""
        execution = Execution()
        service = SpatialProbeService(SpatialProbeGraph(contract), execution)
        token, _ = service.prepare(
            prompt(),
            (SpatialFieldTarget("source", "width"),),
            (SpatialFieldTarget("source", "height"),),
        )
        with pytest.raises(ValueError, match="unavailable"):
            await service.measure(token, "missing")
        first = asyncio.create_task(service.measure(token, "sink/image"))
        await execution.started.wait()
        second = asyncio.create_task(service.measure(token, "sink/image"))
        first.cancel()
        with pytest.raises(asyncio.CancelledError):
            await first
        service.release(token)
        execution.finish.set()
        assert (await second).scale == 2
        assert len(execution.calls) == 1
        with pytest.raises(ValueError, match="expired"):
            await service.measure(token, "sink/image")

    asyncio.run(run())
