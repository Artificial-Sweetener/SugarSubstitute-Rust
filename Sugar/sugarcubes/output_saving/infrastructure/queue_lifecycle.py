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
"""Retire saving snapshots when native queue completion and cancellation remove their jobs."""

from __future__ import annotations

from collections.abc import Callable, Sequence
import logging
from typing import Protocol

_logger = logging.getLogger(__name__)


class RunRetirement(Protocol):
    """Release terminal saving metadata while preserving all saved output files."""

    def discard(self, prompt_id: str) -> None:
        """Retire one terminal or canceled generation without changing host queue outcomes."""


class SavingQueueLifecycle:
    """Observe only native terminal queue operations through transparent owned wrappers."""

    def __init__(self, queue: object, retirement: RunRetirement) -> None:
        """Bind the exact native queue instance and one authoritative run-state owner."""
        self._queue = queue
        self._retirement = retirement
        self._active = True
        self._installed: list[
            tuple[str, Callable[..., object], Callable[..., object]]
        ] = []
        try:
            for name in ("task_done", "wipe_queue", "delete_queue_item"):
                original: object = getattr(queue, name, None)
                if not callable(original):
                    raise RuntimeError(
                        f"Comfy queue lifecycle operation '{name}' is unavailable."
                    )
                wrapper = self._wrap(original)
                setattr(queue, name, wrapper)
                self._installed.append((name, original, wrapper))
        except (AttributeError, RuntimeError, TypeError):
            self.close()
            raise

    def close(self) -> None:
        """Restore only wrappers still owned by this observer and retire retained callbacks."""
        self._active = False
        for name, original, wrapper in reversed(self._installed):
            if getattr(self._queue, name, None) is wrapper:
                setattr(self._queue, name, original)
        self._installed.clear()

    def live_ids(self) -> frozenset[str]:
        """Require authoritative native queue identities before retiring interrupted prior-host runs."""
        snapshot = self._snapshot()
        if snapshot is None:
            raise RuntimeError(
                "Comfy's live queue identities are unavailable for saving-state restoration."
            )
        return snapshot

    def _wrap(self, original: Callable[..., object]) -> Callable[..., object]:
        """Preserve native arguments, return values, exception context and other installed observers."""

        def observed(*args: object, **kwargs: object) -> object:
            """Compare live job identities around one actual native lifecycle transition."""
            if not self._active:
                return original(*args, **kwargs)
            before = self._snapshot()
            try:
                return original(*args, **kwargs)
            finally:
                after = self._snapshot()
                if before is not None and after is not None:
                    for prompt_id in before.difference(after):
                        self._retirement.discard(prompt_id)

        return observed

    def _snapshot(self) -> frozenset[str] | None:
        """Read only job IDs through Comfy's read-safe queue surface without retaining prompt contents."""
        method: object = getattr(self._queue, "get_current_queue_volatile", None)
        if not callable(method):
            method = getattr(self._queue, "get_current_queue", None)
        if not callable(method):
            _logger.error(
                "Output saving lifecycle cannot read the native queue snapshot."
            )
            return None
        try:
            value: object = method()
        except Exception:
            _logger.exception(
                "Output saving lifecycle could not observe native queue identities."
            )
            return None
        if not isinstance(value, tuple) or len(value) != 2:
            _logger.error(
                "Output saving lifecycle received an invalid native queue snapshot."
            )
            return None
        identities: set[str] = set()
        for group in value:
            if not isinstance(group, Sequence) or isinstance(group, str | bytes):
                return None
            for item in group:
                if (
                    isinstance(item, Sequence)
                    and not isinstance(item, str | bytes)
                    and len(item) > 1
                    and isinstance(item[1], str)
                ):
                    identities.add(item[1])
        return frozenset(identities)
