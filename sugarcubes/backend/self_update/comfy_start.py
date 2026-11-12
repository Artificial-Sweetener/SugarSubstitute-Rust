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
"""Observe the end of a full ComfyUI start, the point an update is accepted.

ComfyUI imports every custom node, then starts its server; `start_multi_address`
returns once every address is listening. ComfyUI offers no extension hook after
that point, so this adapter wraps that one server method on the running
instance, after checking it still has the shape it wraps. At that moment the pack's import is proven complete only if ComfyUI
registered every SugarCubes node class, which it does only after the whole
module imported.
"""

from __future__ import annotations

import inspect
import logging
import sys
from collections.abc import Callable, Mapping
from typing import Any

_logger = logging.getLogger("sugarcubes.self_update")
StartReport = Callable[[bool], None]


def comfy_registered_nodes(expected: Mapping[str, object]) -> bool:
    """Return whether ComfyUI registered exactly these SugarCubes node classes."""

    registry = getattr(sys.modules.get("nodes"), "NODE_CLASS_MAPPINGS", None)
    if not isinstance(registry, Mapping) or not expected:
        return False
    return all(registry.get(name) is node for name, node in expected.items())


_EXPECTED_PARAMETERS = ("addresses", "call_on_start", "verbose")
UNOBSERVABLE_START_WARNING = (
    "couldn't confirm ComfyUI finished starting; the update will be confirmed "
    "when the page loads"
)


def _observable_start(server: Any) -> Callable[..., Any] | None:
    """Return ComfyUI's server start method if it still has the shape we wrap."""

    start = getattr(server, "start_multi_address", None)
    if not callable(start) or not inspect.iscoroutinefunction(start):
        return None
    try:
        parameters = tuple(inspect.signature(start).parameters)
    except (TypeError, ValueError):
        return None
    return start if parameters == _EXPECTED_PARAMETERS else None


def watch_comfy_start(
    server: Any, *, expected_nodes: Mapping[str, object], on_started: StartReport
) -> bool:
    """Call `on_started(nodes_registered)` once ComfyUI's server is listening.

    The wrap is installed only when `start_multi_address` is still a coroutine
    taking `(addresses, call_on_start, verbose)`. Returns whether it was
    installed; when it was not, one plain warning is logged and the caller
    falls back to confirming the update when the page loads, so a renamed or
    reshaped ComfyUI method can never roll back a version that started fine.
    """

    start = _observable_start(server)
    if start is None:
        _logger.warning("SugarCubes update: %s", UNOBSERVABLE_START_WARNING)
        return False

    async def start_then_report(
        addresses: Any, call_on_start: Any = None, verbose: bool = True
    ) -> Any:
        result = await start(addresses, call_on_start, verbose)
        on_started(comfy_registered_nodes(expected_nodes))
        return result

    server.start_multi_address = start_then_report
    return True


__all__ = [
    "UNOBSERVABLE_START_WARNING",
    "comfy_registered_nodes",
    "watch_comfy_start",
]
