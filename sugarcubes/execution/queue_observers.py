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
#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Publish validated Cube queue candidates to process-local host integrations."""

from __future__ import annotations

import logging
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from threading import RLock
from typing import Protocol

from .models import ApiPrompt, CubeExecutionReport, JsonValue

_logger = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class ValidatedQueueEvent:
    """Describe one final Cube prompt immediately before native queue insertion."""

    prompt_id: str
    prompt: ApiPrompt
    extra_data: Mapping[str, JsonValue]
    report: CubeExecutionReport
    client_id: str | None


QueueObserver = Callable[[ValidatedQueueEvent], None]


class QueueObserverPublisher(Protocol):
    """Publish one validated queue event or reject a required integration failure."""

    def notify(self, event: ValidatedQueueEvent) -> None:
        """Notify the current observer snapshot."""


class RequiredQueueObserverFailure(RuntimeError):
    """Report that a required host integration rejected queue preparation."""


@dataclass(frozen=True, slots=True)
class _ObserverRegistration:
    """Retain one observer and whether execution requires its success."""

    observer: QueueObserver
    required: bool


class QueueObserverRegistry:
    """Own ordered, idempotent process-local pre-queue observer registration."""

    def __init__(self) -> None:
        """Initialize an empty thread-safe observer registry."""

        self._lock = RLock()
        self._registrations: list[_ObserverRegistration] = []

    def register(self, observer: QueueObserver, *, required: bool = False) -> None:
        """Register an observer once and preserve the strictest requirement level."""

        with self._lock:
            for index, registration in enumerate(self._registrations):
                if registration.observer is observer:
                    if required and not registration.required:
                        self._registrations[index] = _ObserverRegistration(
                            observer=observer,
                            required=True,
                        )
                    return
            self._registrations.append(
                _ObserverRegistration(observer=observer, required=required)
            )

    def unregister(self, observer: QueueObserver) -> None:
        """Remove every registration for one observer identity."""

        with self._lock:
            self._registrations = [
                registration
                for registration in self._registrations
                if registration.observer is not observer
            ]

    def notify(self, event: ValidatedQueueEvent) -> None:
        """Notify a stable snapshot and fail closed only for required observers."""

        with self._lock:
            registrations = tuple(self._registrations)
        for registration in registrations:
            try:
                registration.observer(event)
            except Exception as error:
                _logger.exception(
                    "SugarCubes pre-queue observer failed prompt_id=%s required=%s",
                    event.prompt_id,
                    registration.required,
                )
                if registration.required:
                    raise RequiredQueueObserverFailure(
                        "A required pre-queue observer rejected the execution."
                    ) from error


__all__ = [
    "QueueObserver",
    "QueueObserverPublisher",
    "QueueObserverRegistry",
    "RequiredQueueObserverFailure",
    "ValidatedQueueEvent",
]
