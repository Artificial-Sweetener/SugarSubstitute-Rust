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
"""Project normalized Cube links into frontend connection records."""

from __future__ import annotations

from typing import Any, Dict, Iterator, List, Mapping, Tuple

from .coercion import _coerce_str
from .models import CubeNode, CubeOutputSpec

BINDING_SENTINEL = "@binding"


def _collect_node_connections(nodes: Mapping[str, CubeNode]) -> List[Dict[str, Any]]:
    """Collect node-to-node and binding-to-node connections for the frontend."""

    connections: List[Dict[str, Any]] = []
    for node in nodes.values():
        for input_name, value in node.inputs.items():
            for kind, symbol, slot in _iter_links(value):
                if kind == "binding":
                    connections.append(
                        {
                            "kind": "binding",
                            "from": {"symbol": symbol, "slot": 0},
                            "to": {"symbol": node.symbol, "input": input_name},
                        }
                    )
                else:
                    connections.append(
                        {
                            "kind": "link",
                            "from": {"symbol": symbol, "slot": slot},
                            "to": {"symbol": node.symbol, "input": input_name},
                        }
                    )
    return connections


def _collect_output_connections(
    outputs: Mapping[str, CubeOutputSpec],
) -> List[Dict[str, Any]]:
    """Collect output-marker connections for the frontend payload."""

    connections: List[Dict[str, Any]] = []
    for output in outputs.values():
        connections.append(
            {
                "kind": "output",
                "from": {
                    "symbol": output.source_symbol,
                    "slot": output.source_slot or 0,
                },
                "to": {"symbol": output.alias, "input": "value"},
            }
        )
    return connections


def _iter_links(value: Any) -> Iterator[Tuple[str, str, Any]]:
    """Yield binding and node links embedded inside serialized values."""

    if isinstance(value, list):
        if len(value) == 2 and isinstance(value[0], (str, int)):
            source = value[0]
            slot = value[1]
            if isinstance(source, str) and source == BINDING_SENTINEL:
                alias = _coerce_str(slot)
                if alias:
                    yield ("binding", alias, 0)
            else:
                yield ("node", str(source), slot)
        else:
            for item in value:
                yield from _iter_links(item)
    elif isinstance(value, tuple):
        for item in value:
            yield from _iter_links(item)
    elif isinstance(value, Mapping):
        for item in value.values():
            yield from _iter_links(item)
