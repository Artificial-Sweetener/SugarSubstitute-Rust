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
"""Own bounded captured probe lifetimes and explicit measurement requests."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
from dataclasses import dataclass, field
import logging
from time import monotonic
from typing import Protocol
from uuid import uuid4

from .models import ApiPrompt
from .spatial_probe_graph import SpatialProbeGraph
from .spatial_probe_measurement import infer_spatial_scale
from .spatial_probe_values import (
    SpatialFieldTarget,
    SpatialProbeExecution,
    SpatialProbeMeasurement,
    SpatialProbePlan,
)

_logger = logging.getLogger(__name__)


class SpatialProbeExecutionPort(Protocol):
    """Execute measurements without registering a normal generation or image output."""

    async def run(
        self, execution: SpatialProbeExecution
    ) -> tuple[tuple[int, int], tuple[int, int]]:
        """Return the two actual output frames and retire owned native history."""


@dataclass
class _Capture:
    """Keep one immutable graph snapshot and its deduplicated measurement tasks."""

    plan: SpatialProbePlan
    expires: float
    tasks: dict[str, asyncio.Task[SpatialProbeMeasurement]] = field(
        default_factory=dict
    )


class SpatialProbeService:
    """Separate capture, explicit path choice, execution and resource cleanup from HTTP and UI."""

    def __init__(
        self,
        graph: SpatialProbeGraph,
        execution: SpatialProbeExecutionPort,
        clock: Callable[[], float] = monotonic,
    ) -> None:
        """Bind graph policy and the normal host queue boundary without a reduced backend."""
        self._graph = graph
        self._execution = execution
        self._clock = clock
        self._captures: dict[str, _Capture] = {}
        self._running: set[asyncio.Task[SpatialProbeMeasurement]] = set()

    def prepare(
        self,
        prompt: ApiPrompt,
        width: tuple[SpatialFieldTarget, ...],
        height: tuple[SpatialFieldTarget, ...],
    ) -> tuple[str, SpatialProbePlan]:
        """Retain at most four short-lived graph snapshots without running any nodes."""
        for token, capture in tuple(self._captures.items()):
            if capture.expires < self._clock() and not any(
                not task.done() for task in capture.tasks.values()
            ):
                del self._captures[token]
        if len(self._captures) >= 4:
            raise ValueError(
                "Close an earlier scale discovery before preparing another."
            )
        plan = self._graph.prepare(prompt, width, height)
        token = str(uuid4())
        self._captures[token] = _Capture(plan, self._clock() + 600)
        return token, plan

    async def measure(self, token: str, output: str) -> SpatialProbeMeasurement:
        """Measure only a selected path and share repeated requests for the same captured operation."""
        capture = self._captures.get(token)
        if capture is None or capture.expires < self._clock():
            raise ValueError(
                "The captured scale probe expired; choose discovery again."
            )
        task = capture.tasks.get(output)
        if task is None:
            if self._running:
                raise ValueError(
                    "Another scale probe is running; wait for it to finish."
                )
            execution = self._graph.build(capture.plan, output)
            task = asyncio.create_task(self._measure(execution))
            capture.tasks[output] = task
            self._running.add(task)
            task.add_done_callback(self._settled)
        return await asyncio.shield(task)

    async def _measure(
        self, execution: SpatialProbeExecution
    ) -> SpatialProbeMeasurement:
        """Derive the factor from measured geometry rather than guessed node field names."""
        output = await self._execution.run(execution)
        return infer_spatial_scale(execution.input_sizes, output)

    def _settled(self, task: asyncio.Task[SpatialProbeMeasurement]) -> None:
        """Retire active ownership and preserve diagnostics after an HTTP caller disconnects."""
        self._running.discard(task)
        if not task.cancelled() and task.exception() is not None:
            error = task.exception()
            if isinstance(error, (ValueError, TimeoutError)):
                _logger.warning(
                    "Spatial probe was unavailable", extra={"reason": str(error)}
                )
            else:
                _logger.error("Spatial probe failed", exc_info=error)

    def release(self, token: str) -> None:
        """Release only the caller's snapshot while an accepted native task finishes its cleanup."""
        self._captures.pop(token, None)
