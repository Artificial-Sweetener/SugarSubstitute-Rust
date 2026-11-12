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
instance. At that moment the pack's import is proven complete only if ComfyUI
registered every SugarCubes node class, which it does only after the whole
module imported.
"""

from __future__ import annotations

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


def watch_comfy_start(
    server: Any, *, expected_nodes: Mapping[str, object], on_started: StartReport
) -> bool:
    """Call `on_started(nodes_registered)` once ComfyUI's server is listening.

    Returns whether the server start could be observed. When ComfyUI's server
    no longer has the method, nothing is reported, so an update stays unaccepted
    and the next start restores the previous version rather than trusting a
    start nobody saw complete.
    """

    start = getattr(server, "start_multi_address", None)
    if not callable(start):
        _logger.error(
            "SugarCubes update event=start_unobservable action=keep_previous_version_on_restart"
        )
        return False

    async def start_then_report(*args: Any, **kwargs: Any) -> Any:
        result = await start(*args, **kwargs)
        on_started(comfy_registered_nodes(expected_nodes))
        return result

    server.start_multi_address = start_then_report
    return True


__all__ = ["comfy_registered_nodes", "watch_comfy_start"]
