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
"""Measure execution phases without coupling orchestration to clock mechanics."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field

NanoClock = Callable[[], int]
PhaseTiming = tuple[str, float]


@dataclass
class ExecutionPhaseMeasurements:
    """Own monotonic phase timing state for one execution preparation."""

    _clock_ns: NanoClock
    _last_ns: int
    _timings_ms: list[PhaseTiming] = field(default_factory=list)

    @classmethod
    def start(cls, clock_ns: NanoClock) -> ExecutionPhaseMeasurements:
        """Start one measurement sequence from the supplied monotonic clock."""

        return cls(clock_ns, clock_ns())

    def finish(self, phase: str) -> None:
        """Record one completed phase while preserving declared phase order."""

        current_ns = self._clock_ns()
        if current_ns < self._last_ns:
            raise ValueError("Execution phase clock must be monotonic.")
        elapsed_ms = (current_ns - self._last_ns) / 1_000_000
        self._timings_ms.append((phase, elapsed_ms))
        self._last_ns = current_ns

    def snapshot(self) -> tuple[PhaseTiming, ...]:
        """Return immutable measurements for the public execution report."""

        return tuple(self._timings_ms)

    def total_ms(self) -> float:
        """Return the measured preparation total for structured logging."""

        return sum(duration_ms for _, duration_ms in self._timings_ms)
