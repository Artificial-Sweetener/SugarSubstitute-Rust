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
"""Let a person change the order in which ComfyUI's waiting jobs run."""

from __future__ import annotations

import heapq
import logging
import sys
from typing import Any

from ..authoring.queue_order import reordered_places
from .responses import BackendError, json_error_from_exception, json_success
from .validation import parse_json_body

_LOGGER = logging.getLogger(__name__)
_MAX_NAMED_JOBS = 10_000


def reorder_waiting_jobs(queue: object, order: list[str]) -> list[str]:
    """Reorder ComfyUI's waiting jobs in place and return their identifiers in run order.

    ComfyUI keeps waiting jobs in a heap of tuples whose first item is the job's place and
    whose second is its identifier, and offers no way to move one. The jobs are moved
    here, under the queue's own lock, by exchanging their places: every job keeps its
    identifier, its prompt and the credentials ComfyUI holds for it, which resubmitting a
    job would lose.
    """

    mutex: object = getattr(queue, "mutex", None)
    enter: object = getattr(mutex, "__enter__", None)
    leave: object = getattr(mutex, "__exit__", None)
    if not callable(enter) or not callable(leave):
        raise BackendError("ComfyUI's queue is unavailable", status=503)
    enter()
    try:
        items: object = getattr(queue, "queue", None)
        if not isinstance(items, list):
            raise BackendError("ComfyUI's queue is unavailable", status=503)
        waiting: list[tuple[float, str]] = []
        for item in items:
            if (
                not isinstance(item, tuple)
                or len(item) < 2
                or isinstance(item[0], bool)
                or not isinstance(item[0], (int, float))
                or not isinstance(item[1], str)
            ):
                # An entry of a shape this code does not know must not be reordered blind.
                raise BackendError(
                    "ComfyUI's queue holds jobs in a form that cannot be reordered",
                    status=409,
                )
            waiting.append((float(item[0]), item[1]))
        places = reordered_places(waiting, order)
        if places:
            for index, item in enumerate(items):
                place = places.get(item[1])
                if place is not None:
                    items[index] = (type(item[0])(place), *item[1:])
            heapq.heapify(items)
        result = [job for _, job in sorted((item[0], item[1]) for item in items)]
    finally:
        leave(None, None, None)
    if places:
        server: object = getattr(queue, "server", None)
        updated: object = getattr(server, "queue_updated", None)
        if callable(updated):
            updated()
        _LOGGER.info(
            "Waiting jobs reordered",
            extra={"operation": "queue_reorder", "moved": len(places)},
        )
    return result


def _comfy_queue() -> object:
    """Find the running ComfyUI's queue, which only its server module can name."""

    # ComfyUI loads its server module under this name; outside ComfyUI it is absent.
    server: object = sys.modules.get("server")
    instance: object = getattr(getattr(server, "PromptServer", None), "instance", None)
    queue: object = getattr(instance, "prompt_queue", None)
    if queue is None:
        raise BackendError("ComfyUI's queue is unavailable", status=503)
    return queue


async def reorder_queue_route(request: Any) -> Any:
    """Run the waiting jobs named in ``order`` in that order, soonest first."""

    try:
        body = await parse_json_body(request)
        order = body.get("order")
        if (
            not isinstance(order, list)
            or len(order) > _MAX_NAMED_JOBS
            or not all(isinstance(job, str) and job for job in order)
        ):
            raise BackendError("'order' must be a list of job identifiers", status=400)
        return json_success({"order": reorder_waiting_jobs(_comfy_queue(), order)})
    except BackendError as error:
        return json_error_from_exception(error)


__all__ = ["reorder_queue_route", "reorder_waiting_jobs"]
