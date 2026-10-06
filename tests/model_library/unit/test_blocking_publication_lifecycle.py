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
"""Prove cancelled callers drain blocking work and retain diagnostic failure context."""

from __future__ import annotations

import asyncio
from threading import Event
import pytest

from sugarcubes.model_library.application.blocking_model_work import (
    complete_blocking_model_work,
)


def test_blocking_failure_after_cancellation_remains_observable(
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Report the original worker exception even when the request returns cancellation."""
    started, release = Event(), Event()

    def operation() -> None:
        """Expose a bounded blocking failure after the caller cancels its waiter."""
        started.set()
        if not release.wait(5):
            raise RuntimeError("Proof did not release the worker.")
        raise ValueError("fixture physical publication failed")

    async def exercise() -> None:
        """Retain the actual worker task until its exception is observed."""
        task = asyncio.create_task(complete_blocking_model_work(operation))
        try:
            assert await asyncio.to_thread(started.wait, 5)
            task.cancel()
            await asyncio.sleep(0)
            assert not task.done()
        finally:
            release.set()
            with pytest.raises(asyncio.CancelledError):
                await task

    asyncio.run(exercise())
    assert len(caplog.records) == 1
    assert caplog.records[0].exc_info is not None
    assert isinstance(caplog.records[0].exc_info[1], ValueError)
    assert str(caplog.records[0].exc_info[1]) == "fixture physical publication failed"
