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
"""Normalize Comfy's flattened native-subgraph execution nodes for export."""

from __future__ import annotations

from typing import Any, Iterable, MutableMapping


def remove_nested_execution_nodes(
    prompt: MutableMapping[str, Any],
    *,
    instance_id: str,
    immediate_node_ids: Iterable[str],
) -> None:
    """Remove descendant executions while retaining immediate wrapper nodes.

    Comfy flattens nested native subgraphs into colon-delimited prompt node IDs.
    SugarCube persistence retains the immediate wrapper and stores its body in
    ``implementation.subgraphs``, so descendant executions must not also become
    root ``implementation.nodes`` entries.
    """

    prefix = f"{instance_id}:"
    retained = set(immediate_node_ids)
    descendants = [
        node_id
        for raw_node_id in prompt
        if (node_id := str(raw_node_id)).startswith(prefix) and node_id not in retained
    ]
    for node_id in descendants:
        prompt.pop(node_id, None)
