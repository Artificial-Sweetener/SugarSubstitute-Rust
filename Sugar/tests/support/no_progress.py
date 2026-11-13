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
"""Wait on observed test events under one far hang guard, never a short deadline.

No test passes or fails on elapsed seconds. A test waits for the event it is
about; the guard exists only so a deadlocked test ends the run with a clear
message instead of hanging the gate forever. It is deliberately far beyond any
honest wait on a slow, loaded machine.
"""

from __future__ import annotations

import asyncio
import threading
from collections.abc import Awaitable
from concurrent.futures import Future
from typing import TypeVar

HANG_GUARD_SECONDS = 600.0
_Value = TypeVar("_Value")


class HangGuardExpired(AssertionError):
    """Report a test that made no progress for the whole hang guard."""


def _expired(what: str) -> HangGuardExpired:
    return HangGuardExpired(
        f"No progress for {HANG_GUARD_SECONDS:.0f} s while waiting for {what}; "
        "the test is deadlocked."
    )


def wait_for_event(event: threading.Event, what: str = "an event") -> None:
    """Block until a thread event is set."""

    if not event.wait(HANG_GUARD_SECONDS):
        raise _expired(what)


def join_thread(thread: threading.Thread, what: str = "a thread to finish") -> None:
    """Block until a thread ends."""

    thread.join(HANG_GUARD_SECONDS)
    if thread.is_alive():
        raise _expired(what)


def future_result(future: Future[_Value], what: str = "a result") -> _Value:
    """Return a future's result once it is ready."""

    try:
        return future.result(HANG_GUARD_SECONDS)
    except TimeoutError as exc:
        raise _expired(what) from exc


async def await_outcome(
    awaitable: Awaitable[_Value], what: str = "an outcome"
) -> _Value:
    """Await one coroutine or task to completion."""

    try:
        return await asyncio.wait_for(awaitable, HANG_GUARD_SECONDS)
    except TimeoutError as exc:
        raise _expired(what) from exc


__all__ = [
    "HANG_GUARD_SECONDS",
    "HangGuardExpired",
    "await_outcome",
    "future_result",
    "join_thread",
    "wait_for_event",
]
