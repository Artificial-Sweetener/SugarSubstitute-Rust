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
"""Prove waiting jobs are reordered in place, without losing or inventing any."""

from __future__ import annotations

import heapq
import threading

import pytest

from sugarcubes.authoring.queue_order import reordered_places
from sugarcubes.backend.queue_order_routes import reorder_waiting_jobs
from sugarcubes.backend.responses import BackendError


class _Server:
    def __init__(self) -> None:
        self.updates = 0

    def queue_updated(self) -> None:
        self.updates += 1


class _Queue:
    """ComfyUI's queue as far as reordering touches it: a locked heap of job tuples."""

    def __init__(self, jobs: list[tuple[object, ...]]) -> None:
        self.mutex = threading.RLock()
        self.queue = list(jobs)
        heapq.heapify(self.queue)
        self.server = _Server()

    def run_order(self) -> list[str]:
        return [str(item[1]) for item in sorted(self.queue)]


def test_named_jobs_exchange_the_places_they_already_hold() -> None:
    waiting = [(3.0, "a"), (4.0, "b"), (7.0, "c"), (9.0, "d")]

    assert reordered_places(waiting, ["d", "a", "c", "b"]) == {
        "d": 3.0,
        "a": 4.0,
        "b": 9.0,
    }
    # A job that is not named keeps its place, and the others move around it.
    assert reordered_places(waiting, ["c", "a"]) == {"c": 3.0, "a": 7.0}
    assert reordered_places(waiting, ["a", "b", "c", "d"]) == {}


def test_names_that_no_longer_wait_and_repeats_are_ignored() -> None:
    waiting = [(1.0, "a"), (2.0, "b")]

    assert reordered_places(waiting, ["gone", "b", "b", "a"]) == {"b": 1.0, "a": 2.0}
    assert reordered_places(waiting, []) == {}
    assert reordered_places([], ["a"]) == {}


def test_the_queue_is_reordered_in_place_and_keeps_every_job_whole() -> None:
    queue = _Queue(
        [
            (1, "a", {"prompt": 1}, {"extra": "a"}, ["9"], {"secret": "kept"}),
            (2, "b", {"prompt": 2}, {}, [], {}),
            (3, "c", {"prompt": 3}, {}, [], {}),
        ]
    )

    assert reorder_waiting_jobs(queue, ["c", "a", "b"]) == ["c", "a", "b"]
    assert queue.run_order() == ["c", "a", "b"]
    moved = next(item for item in queue.queue if item[1] == "a")
    assert moved == (2, "a", {"prompt": 1}, {"extra": "a"}, ["9"], {"secret": "kept"})
    # The heap is still a heap, so ComfyUI takes the jobs in the new order.
    assert [heapq.heappop(queue.queue)[1] for _ in range(3)] == ["c", "a", "b"]
    assert queue.server.updates == 1


def test_an_unchanged_order_tells_nobody_and_an_unknown_shape_is_refused() -> None:
    queue = _Queue([(1, "a"), (2, "b")])

    assert reorder_waiting_jobs(queue, ["a", "b"]) == ["a", "b"]
    assert queue.server.updates == 0

    strange = _Queue([(1, "a")])
    strange.queue.append(("not a place", "b"))
    with pytest.raises(BackendError) as refused:
        reorder_waiting_jobs(strange, ["b", "a"])
    assert refused.value.status == 409
    assert strange.queue == [(1, "a"), ("not a place", "b")]

    with pytest.raises(BackendError) as missing:
        reorder_waiting_jobs(object(), ["a"])
    assert missing.value.status == 503
