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
"""Runtime materialization helpers for canonical cube documents."""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Mapping, Optional

from .document import CubeDocument


def materialize_nodes(
    document: CubeDocument,
    *,
    authored_flavor_id: Optional[str] = None,
    local_values: Optional[Mapping[str, Any]] = None,
    live_values: Optional[Mapping[str, Any]] = None,
) -> dict[str, dict[str, Any]]:
    """Overlay flavor and live values onto implementation nodes for runtime use."""

    nodes = deepcopy(document.implementation.nodes)
    authored_values = _resolve_authored_values(document, authored_flavor_id)
    _apply_control_values(nodes, document, authored_values)
    _apply_control_values(nodes, document, local_values or {})
    _apply_control_values(nodes, document, live_values or {})
    return nodes


def _resolve_authored_values(
    document: CubeDocument,
    authored_flavor_id: Optional[str],
) -> Mapping[str, Any]:
    """Resolve the authored flavor values that should seed runtime nodes."""

    flavor_id = authored_flavor_id or document.surface.default_flavor_id
    for flavor in document.flavors.authored:
        if flavor.id == flavor_id:
            return flavor.values
    return {}


def _apply_control_values(
    nodes: dict[str, dict[str, Any]],
    document: CubeDocument,
    values: Mapping[str, Any],
) -> None:
    """Apply resolved control values onto the runtime node payload."""

    if not values:
        return
    control_index = {
        control.control_id: control for control in document.surface.controls
    }
    for control_id, value in values.items():
        control = control_index.get(control_id)
        if control is None:
            continue
        node_payload = nodes.get(control.symbol)
        if not isinstance(node_payload, dict):
            continue
        inputs = node_payload.get("inputs")
        if not isinstance(inputs, dict):
            inputs = {}
            node_payload["inputs"] = inputs
        inputs[control.input_name] = deepcopy(value)
