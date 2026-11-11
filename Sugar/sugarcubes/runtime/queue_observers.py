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
"""Own the process-local validated-queue observer registry."""

from __future__ import annotations

from ..execution.queue_observers import QueueObserver, QueueObserverRegistry

QUEUE_OBSERVER_API_VERSION = 1
_QUEUE_OBSERVERS = QueueObserverRegistry()


def queue_observer_registry() -> QueueObserverRegistry:
    """Return the single observer registry used by the active execution port."""

    return _QUEUE_OBSERVERS


def register_validated_queue_observer(
    observer: QueueObserver,
    *,
    required: bool = False,
) -> None:
    """Register one sibling-extension observer for validated Cube prompts."""

    _QUEUE_OBSERVERS.register(observer, required=required)


def unregister_validated_queue_observer(observer: QueueObserver) -> None:
    """Remove one sibling-extension queue observer by identity."""

    _QUEUE_OBSERVERS.unregister(observer)


__all__ = [
    "QUEUE_OBSERVER_API_VERSION",
    "queue_observer_registry",
    "register_validated_queue_observer",
    "unregister_validated_queue_observer",
]
