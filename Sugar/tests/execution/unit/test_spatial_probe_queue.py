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
"""Prove native spatial measurements isolate execution and retire only their owned records."""

from __future__ import annotations

import asyncio
from collections.abc import Callable

import pytest

from sugarcubes.execution.comfy_spatial_probe import ComfySpatialProbe
from sugarcubes.execution.models import ComfyQueueReceipt, PreparedCubeExecution
from sugarcubes.execution.spatial_measurement_node import SpatialProbeMeasurementNode
from sugarcubes.execution.spatial_probe_graph import MEASUREMENT_CLASS, MEASUREMENT_NODE
from sugarcubes.execution.spatial_probe_registry import SpatialProbeRegistry
from sugarcubes.execution.spatial_probe_values import SpatialProbeExecution


class NativeQueue:
    """Model the external native queue, including completion between two observation calls."""

    def __init__(self) -> None:
        """Retain unrelated history so cleanup ownership is observable."""
        self.history: dict[str, object] = {"user-job": {"outputs": {"saved": "image"}}}
        self.jobs: list[object] = []
        self.finish_on_observation = False
        self.deleted: list[str] = []
        self.cleaned = asyncio.Event()

    def finish(self) -> None:
        """Publish the measurement and remove its running queue entry atomically."""
        self.jobs.clear()
        self.history["probe"] = {
            "outputs": {
                MEASUREMENT_NODE: {"spatial_dimensions": [[256, 128], [384, 256]]}
            }
        }

    def get_current_queue(self) -> tuple[list[object], list[object]]:
        """Allow completion after an empty history read, as in the real worker thread."""
        if self.finish_on_observation and self.jobs:
            self.finish()
        return self.jobs.copy(), []

    def get_history(self, prompt_id: str) -> object:
        """Return only the requested native history record."""
        return {prompt_id: self.history[prompt_id]} if prompt_id in self.history else {}

    def delete_history_item(self, prompt_id: str) -> None:
        """Record exact-job deletion without touching normal generations."""
        self.deleted.append(prompt_id)
        self.history.pop(prompt_id, None)
        self.cleaned.set()

    def delete_queue_item(self, match: Callable[[object], bool]) -> bool:
        """Remove only matching pending jobs; this fixture models all jobs as running."""
        assert not match((1, "user-job"))
        return False


class Execution:
    """Replace only native validation and scheduling, retaining the production probe owner."""

    def __init__(self, queue: NativeQueue) -> None:
        """Bind one observable native queue and default to synchronous completion."""
        self.native = queue
        self.prepared: list[PreparedCubeExecution] = []
        self.accept = True
        self.complete = True

    async def queue(self, prepared: PreparedCubeExecution) -> ComfyQueueReceipt:
        """Expose the exact scheduling payload and emulate host acceptance or rejection."""
        self.prepared.append(prepared)
        if self.accept:
            self.native.jobs.append((1, "probe"))
            if self.complete:
                self.native.finish()
        return ComfyQueueReceipt(self.accept, "probe", 1)


def fixture() -> (
    tuple[NativeQueue, Execution, ComfySpatialProbe, SpatialProbeExecution]
):
    """Keep the actual measurement schema and a minimal executable probe."""
    queue = NativeQueue()
    execution = Execution(queue)
    registry = SpatialProbeRegistry(
        lambda: {MEASUREMENT_CLASS: SpatialProbeMeasurementNode}
    )
    probe = ComfySpatialProbe(queue, execution, registry)
    plan = SpatialProbeExecution(
        {MEASUREMENT_NODE: {"class_type": MEASUREMENT_CLASS, "inputs": {}}},
        MEASUREMENT_NODE,
        ((128, 64), (192, 128)),
    )
    return queue, execution, probe, plan


def test_native_probe_is_measurement_only_and_removes_its_history() -> None:
    """Keep outputs and client identity separate from generation delivery and retain user history."""
    queue, execution, probe, plan = fixture()
    assert asyncio.run(probe.run(plan)) == ((256, 128), (384, 256))
    prepared = execution.prepared[0]
    assert prepared.workflow == {}
    assert prepared.queue.client_id is not None
    assert prepared.queue.partial_execution_targets == (MEASUREMENT_NODE,)
    assert prepared.extra_data == {"sugarcubes_spatial_probe": True}
    assert set(queue.history) == {"user-job"}
    assert queue.deleted == ["probe"]


def test_completion_between_history_and_queue_observation_is_not_a_failure() -> None:
    """Account for atomic native completion without resubmitting or retrying execution."""
    queue, execution, probe, plan = fixture()
    execution.complete = False
    queue.finish_on_observation = True
    assert asyncio.run(probe.run(plan)) == ((256, 128), (384, 256))
    assert len(execution.prepared) == 1
    assert set(queue.history) == {"user-job"}


def test_busy_or_rejected_native_queue_does_not_modify_unrelated_jobs() -> None:
    """Refuse discovery while generation is busy and surface native validation rejection."""
    queue, execution, probe, plan = fixture()
    queue.jobs.append((0, "user-job"))
    with pytest.raises(ValueError, match="busy"):
        asyncio.run(probe.run(plan))
    assert execution.prepared == []
    queue.jobs.clear()
    execution.accept = False
    with pytest.raises(ValueError, match="rejected"):
        asyncio.run(probe.run(plan))
    assert queue.deleted == [] and set(queue.history) == {"user-job"}


def test_cancelled_observation_finishes_owned_cleanup_without_interrupting_comfy() -> (
    None
):
    """Let an accepted native job settle and remove its history after the caller disconnects."""

    async def run() -> None:
        """Cancel only observation, then deterministically complete the running external job."""
        queue, execution, probe, plan = fixture()
        execution.complete = False
        task = asyncio.create_task(probe.run(plan))
        await asyncio.sleep(0)
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task
        assert queue.jobs == [(1, "probe")]
        queue.finish()
        await asyncio.wait_for(queue.cleaned.wait(), timeout=1)
        assert set(queue.history) == {"user-job"}

    asyncio.run(run())


@pytest.mark.parametrize(
    "result", [{}, {"outputs": {}}, {"status": {"status_str": "error"}}]
)
def test_invalid_native_measurement_is_not_treated_as_scale(result: object) -> None:
    """Fail closed for missing, malformed and errored native history records."""
    with pytest.raises(ValueError):
        ComfySpatialProbe._measurements(result, MEASUREMENT_NODE)
