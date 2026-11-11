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
"""Prove completion and cancellation retire only removed jobs while preserving native behavior."""

from __future__ import annotations

from sugarcubes.output_saving.infrastructure.queue_lifecycle import SavingQueueLifecycle
import pytest


class Queue:
    """Expose native queue transitions without allowing observers to alter their results."""

    def __init__(self) -> None:
        """Keep independent running and pending fixture identities."""
        self.running = [(0, "running")]
        self.pending = [(1, "pending")]

    def get_current_queue_volatile(
        self,
    ) -> tuple[list[tuple[int, str]], list[tuple[int, str]]]:
        """Return read-only copies of the current job identities."""
        return list(self.running), list(self.pending)

    def task_done(self, item_id: object, history: object, *, status: object) -> str:
        """Complete the running job and preserve positional and keyword arguments."""
        assert item_id == 0 and history == {"outputs": {}} and status == "success"
        self.running.clear()
        return "done"

    def wipe_queue(self) -> str:
        """Clear pending jobs while leaving the running generation untouched."""
        self.pending.clear()
        return "wiped"

    def delete_queue_item(self, selected: object) -> bool:
        """Cancel one known pending job through the native operation."""
        assert selected == "pending"
        self.pending.clear()
        return True


class Retirement:
    """Record terminal state retirement separately from queue mutation."""

    def __init__(self) -> None:
        """Capture only retired prompt IDs."""
        self.ids: list[str] = []

    def discard(self, prompt_id: str) -> None:
        """Record the exact generation whose queue transition is already terminal."""
        self.ids.append(prompt_id)


def test_completion_preserves_native_results_and_keeps_pending_policy() -> None:
    """Retire metadata only after native completion has removed the running job."""
    queue, retirement = Queue(), Retirement()
    original = queue.task_done
    observer = SavingQueueLifecycle(queue, retirement)
    assert queue.task_done(0, {"outputs": {}}, status="success") == "done"
    assert retirement.ids == ["running"]
    assert queue.pending == [(1, "pending")]
    observer.close()
    assert queue.task_done == original


def test_pending_clear_does_not_retire_the_running_generation() -> None:
    """Keep active image sinks usable while the user clears queued work."""
    queue, retirement = Queue(), Retirement()
    observer = SavingQueueLifecycle(queue, retirement)
    assert queue.wipe_queue() == "wiped"
    assert retirement.ids == ["pending"]
    assert queue.running == [(0, "running")]
    observer.close()


def test_single_cancel_preserves_native_result_and_releases_exact_policy() -> None:
    """Observe cancellation without intercepting the caller's selection argument."""
    queue, retirement = Queue(), Retirement()
    observer = SavingQueueLifecycle(queue, retirement)
    assert queue.delete_queue_item("pending")
    assert retirement.ids == ["pending"]
    observer.close()


def test_failed_external_snapshot_keeps_native_queue_operation_and_logs_failure(
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Never guess retired jobs or block native cancellation when another observer's snapshot fails."""

    class ExternalFailure(Exception):
        """Model an arbitrary exception type supplied by a host extension."""

    class UnobservableQueue(Queue):
        """Keep native mutation operational while rejecting external snapshot inspection."""

        def get_current_queue_volatile(
            self,
        ) -> tuple[list[tuple[int, str]], list[tuple[int, str]]]:
            """Raise at the external read boundary before any identities can be trusted."""
            raise ExternalFailure("fixture snapshot failure")

    queue, retirement = UnobservableQueue(), Retirement()
    observer = SavingQueueLifecycle(queue, retirement)
    assert queue.wipe_queue() == "wiped"
    assert retirement.ids == []
    assert "could not observe native queue identities" in caplog.text
    with pytest.raises(RuntimeError):
        observer.live_ids()
    observer.close()
