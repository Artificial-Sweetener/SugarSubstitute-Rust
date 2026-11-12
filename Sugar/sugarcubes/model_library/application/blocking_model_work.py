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
"""Drain blocking model publication when a caller cancels its asynchronous request."""

from __future__ import annotations

import asyncio
from collections.abc import Callable
import logging
from typing import ParamSpec, TypeVar

_PARAMETERS = ParamSpec("_PARAMETERS")
_RESULT = TypeVar("_RESULT")
_LOGGER = logging.getLogger(__name__)


async def complete_blocking_model_work(
    operation: Callable[_PARAMETERS, _RESULT],
    *args: _PARAMETERS.args,
    **kwargs: _PARAMETERS.kwargs,
) -> _RESULT:
    """Keep cancellation pending until the worker finishes, preserving physical publication leases.

    Cancelling an asyncio waiter cannot stop an already running thread. Retain its
    task, observe its outcome and return cancellation only after the work ends.
    """
    task = asyncio.create_task(asyncio.to_thread(operation, *args, **kwargs))
    try:
        return await asyncio.shield(task)
    except asyncio.CancelledError:
        while not task.done():
            try:
                await asyncio.wait((task,))
            except asyncio.CancelledError:
                continue
        if not task.cancelled():
            error = task.exception()
            if error is not None:
                _LOGGER.error(
                    "Blocking model work failed while its caller was cancelled",
                    exc_info=(type(error), error, error.__traceback__),
                )
        raise
