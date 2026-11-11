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
"""Execute bounded spatial experiments through the authoritative Comfy queue adapter."""

from __future__ import annotations

import asyncio
from collections.abc import Callable, Mapping, Sequence
from hashlib import sha256
import json
import logging
from time import monotonic
from typing import Protocol
from uuid import uuid4

from .models import (
    CubeExecutionReport,
    CubeQueueOptions,
    ExecutionNodeOwner,
    PreparedCubeExecution,
)
from .ports import ComfyExecutionPort
from .spatial_probe_registry import SpatialProbeRegistry
from .spatial_probe_values import SpatialProbeExecution

_logger = logging.getLogger(__name__)


class SpatialNativeQueue(Protocol):
    """Expose only native queue observation and cleanup of an exact owned job."""

    def get_current_queue(self) -> tuple[list[object], list[object]]:
        """Return currently running and pending native jobs."""

    def get_history(self, prompt_id: str) -> object:
        """Read one job's terminal measurement."""

    def delete_history_item(self, prompt_id: str) -> None:
        """Remove only the completed measurement owned by this operation."""

    def delete_queue_item(self, match: Callable[[object], bool]) -> bool:
        """Remove an owned job only if it has not started."""


class ComfySpatialProbe:
    """Own native measurement queueing, terminal observation and exact-job cleanup."""

    def __init__(
        self,
        queue: SpatialNativeQueue,
        execution: ComfyExecutionPort,
        registry: SpatialProbeRegistry,
    ) -> None:
        """Reuse normal Comfy validation and scheduling with image saving disabled for measurements."""
        self._queue = queue
        self._execution = execution
        self._registry = registry
        self._late_cleanup: set[asyncio.Task[None]] = set()

    async def run(
        self, execution: SpatialProbeExecution
    ) -> tuple[tuple[int, int], tuple[int, int]]:
        """Queue one small experiment on the full backend and return no ordinary generation media."""
        running, waiting = self._queue.get_current_queue()
        if running or waiting:
            raise ValueError(
                "ComfyUI is busy; run scale discovery when the generation queue is idle."
            )
        definitions: dict[str, Mapping[str, object]] = {}
        for node_id, node in execution.prompt.items():
            class_type = node.get("class_type")
            if not isinstance(class_type, str):
                raise ValueError("The spatial probe has an invalid native class.")
            definitions[node_id] = self._registry.definition(class_type)
        digest = sha256(
            json.dumps(execution.prompt, sort_keys=True, allow_nan=False).encode()
        ).hexdigest()
        report = CubeExecutionReport(
            schema_version=1,
            phase_order=("spatial-probe",),
            workflow_semantic_hash=digest,
            topology_edges=(),
            topology_components=(),
            phase_timings_ms=(),
        )
        prepared = PreparedCubeExecution(
            prompt=execution.prompt,
            workflow={},
            node_definitions=definitions,
            node_owners={
                node_id: ExecutionNodeOwner(None) for node_id in execution.prompt
            },
            boundary_bindings=(),
            queue=CubeQueueOptions(
                client_id=str(uuid4()),
                partial_execution_targets=(execution.measurement_node,),
            ),
            extra_data={"sugarcubes_spatial_probe": True},
            report=report,
        )
        receipt = await self._execution.queue(prepared)
        if not receipt.accepted:
            raise ValueError(
                "ComfyUI rejected the bounded spatial probe; its nodes or inputs are unsupported."
            )
        prompt_id = receipt.prompt_id
        _logger.info(
            "Spatial probe queued",
            extra={"prompt_id": prompt_id, "node_count": len(execution.prompt)},
        )
        deadline = monotonic() + 90
        try:
            while monotonic() < deadline:
                history = self._queue.get_history(prompt_id)
                if isinstance(history, Mapping) and prompt_id in history:
                    return self._measurements(
                        history[prompt_id], execution.measurement_node
                    )
                if not self._pending(prompt_id):
                    # The native worker publishes history before removing the running job.
                    history = self._queue.get_history(prompt_id)
                    if isinstance(history, Mapping) and prompt_id in history:
                        return self._measurements(
                            history[prompt_id], execution.measurement_node
                        )
                    raise ValueError(
                        "The native spatial probe ended without a measurement."
                    )
                await asyncio.sleep(0.1)
            raise TimeoutError("The bounded scale probe did not complete in time.")
        finally:
            self._queue.delete_queue_item(lambda item: self._job_id(item) == prompt_id)
            if self._pending(prompt_id):
                task = asyncio.create_task(self._cleanup_when_finished(prompt_id))
                self._late_cleanup.add(task)
                task.add_done_callback(self._cleanup_settled)
            else:
                self._queue.delete_history_item(prompt_id)

    def _pending(self, prompt_id: str) -> bool:
        """Observe the accepted job rather than retrying or resubmitting native execution."""
        running, waiting = self._queue.get_current_queue()
        return any(self._job_id(item) == prompt_id for item in (*running, *waiting))

    @staticmethod
    def _job_id(value: object) -> str | None:
        """Narrow Comfy's external queue tuple before matching owned cleanup."""
        if (
            isinstance(value, Sequence)
            and not isinstance(value, (str, bytes))
            and len(value) > 1
        ):
            identifier: object = value[1]
            return identifier if isinstance(identifier, str) else None
        return None

    async def _cleanup_when_finished(self, prompt_id: str) -> None:
        """Retire a still-running measurement after timeout without interrupting the user's generation engine."""
        while self._pending(prompt_id):
            await asyncio.sleep(0.25)
        self._queue.delete_history_item(prompt_id)

    def _cleanup_settled(self, task: asyncio.Task[None]) -> None:
        """Preserve diagnostics for cleanup that outlives its HTTP caller."""
        self._late_cleanup.discard(task)
        if not task.cancelled() and task.exception() is not None:
            _logger.error(
                "Late spatial probe cleanup failed", exc_info=task.exception()
            )

    @staticmethod
    def _measurements(
        value: object, node_id: str
    ) -> tuple[tuple[int, int], tuple[int, int]]:
        """Validate the dedicated measurement result rather than accepting image-output metadata."""
        if not isinstance(value, Mapping):
            raise ValueError("The native spatial probe returned an invalid result.")
        status: object = value.get("status")
        if isinstance(status, Mapping) and status.get("status_str") == "error":
            raise ValueError("ComfyUI could not execute this spatial probe path.")
        outputs: object = value.get("outputs")
        result: object = outputs.get(node_id) if isinstance(outputs, Mapping) else None
        frames: object = (
            result.get("spatial_dimensions") if isinstance(result, Mapping) else None
        )
        if not isinstance(frames, list) or len(frames) != 2:
            raise ValueError(
                "The output did not produce exactly two measurable probe frames."
            )
        decoded: list[tuple[int, int]] = []
        for frame in frames:
            if (
                not isinstance(frame, list)
                or len(frame) != 2
                or not all(type(edge) is int and edge > 0 for edge in frame)
            ):
                raise ValueError("The output returned invalid frame dimensions.")
            width: object = frame[0]
            height: object = frame[1]
            if not isinstance(width, int) or not isinstance(height, int):
                raise ValueError("The output returned invalid frame dimensions.")
            decoded.append((width, height))
        return decoded[0], decoded[1]
