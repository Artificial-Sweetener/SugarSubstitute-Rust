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
