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
"""Bound each output inspection by cancellation, deadline, candidate and metadata admission."""

from collections.abc import Callable
from dataclasses import dataclass, field
from threading import Event
from time import monotonic

from ..domain.artifact import ArtifactUnavailable


@dataclass(slots=True)
class ArtifactBudget:
    """Charge work in the worker that owns its file handles, including after HTTP cancellation."""

    cancelled: Event = field(default_factory=Event)
    clock: Callable[[], float] = monotonic
    seconds: float = 15.0
    maximum_candidates: int = 10000
    maximum_metadata: int = 64 * 1024 * 1024
    _deadline: float = field(init=False)
    _candidates: int = 0
    _metadata: int = 0

    def __post_init__(self) -> None:
        """Start a finite deadline when admitted, rather than while queued."""
        self._deadline = self.clock() + self.seconds

    def check(self) -> None:
        """Reject cancelled or expired work before its next bounded unit of I/O."""
        if self.cancelled.is_set():
            raise ArtifactUnavailable("cancelled", "Output inspection was cancelled.")
        if (
            self._metadata > self.maximum_metadata
            or self._candidates > self.maximum_candidates
        ):
            raise ArtifactUnavailable(
                "budget", "Output inspection exceeded its admitted work limit."
            )
        if self.clock() >= self._deadline:
            raise ArtifactUnavailable(
                "budget", "Output inspection exceeded its time limit."
            )

    def candidate(self) -> None:
        """Count every enumerated filesystem entry, including unrelated or unsupported files."""
        self.check()
        self._candidates += 1
        if self._candidates > self.maximum_candidates:
            raise ArtifactUnavailable(
                "budget", "Output discovery exceeded its file limit."
            )

    def metadata(self, count: int) -> None:
        """Charge cumulative header reads across all files, not only successful candidates."""
        self.check()
        self._metadata += count
        if self._metadata > self.maximum_metadata:
            raise ArtifactUnavailable(
                "budget", "Output discovery exceeded its metadata limit."
            )
