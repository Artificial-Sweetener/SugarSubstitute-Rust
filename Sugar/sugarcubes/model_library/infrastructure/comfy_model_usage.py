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
"""Observe accepted native queue insertion for both normal and SugarCube dispatches."""

from __future__ import annotations

from collections.abc import Callable
import logging
from typing import cast

from ..application.queued_model_usage import QueuedModelUsageRecorder

_logger = logging.getLogger(__name__)


class ComfyModelUsageObserver:
    """Keep the host's queue authoritative and count usage only after successful insertion."""

    def __init__(self, queue: object, recorder: QueuedModelUsageRecorder) -> None:
        """Adapt Comfy's instance queue method without changing validation or queue contents."""
        original: object = getattr(queue, "put", None)
        if not callable(original):
            raise RuntimeError(
                "Comfy's prompt queue insertion boundary is unavailable."
            )
        self.queue = queue
        self.recorder = recorder
        # Comfy's put(item) method is dynamically supplied by the host, not a typed package API.
        self.original = cast(Callable[[object], object], original)
        self.active = True

        def put(item: object) -> object:
            """Delegate before observation so rejected insertions never count as usage."""
            result = self.original(item)
            if self.active:
                self._observe_accepted(item)
            return result

        self.installed = put
        setattr(queue, "put", put)

    def _observe_accepted(self, item: object) -> None:
        """Contain and diagnose optional observer faults after irreversible host acceptance."""
        try:
            self.recorder.record_values(_queued_values(item))
        except Exception:
            # The native queue already accepted this item; metadata faults must not report rejection.
            _logger.exception(
                "Model usage could not be recorded after accepted queue insertion"
            )

    def close(self) -> None:
        """Deactivate retained wrapper chains and restore only the method this adapter still owns."""
        self.active = False
        if getattr(self.queue, "put", None) is self.installed:
            setattr(self.queue, "put", self.original)


def _queued_values(item: object) -> frozenset[str]:
    """Narrow only the accepted queue tuple's prompt, excluding extra-data strings and credentials."""
    if not isinstance(item, tuple) or len(item) < 3 or not isinstance(item[2], dict):
        raise ValueError("Comfy returned an unsupported accepted prompt queue item.")
    values: set[str] = set()
    pending: list[object] = [item[2]]
    seen: set[int] = set()
    while pending:
        value = pending.pop()
        if isinstance(value, str):
            values.add(value)
        elif isinstance(value, (dict, list, tuple)) and id(value) not in seen:
            seen.add(id(value))
            pending.extend(value.values() if isinstance(value, dict) else value)
    return frozenset(values)
