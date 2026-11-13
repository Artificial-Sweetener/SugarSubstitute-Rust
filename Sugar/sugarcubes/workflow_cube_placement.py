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
"""Own native Cube reading order and collision-safe permutation geometry."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from math import isfinite

from .workflow_mutation_errors import CubeGraphMutationError

MINIMUM_CUBE_GAP = 24.0


def read_cube_pair(value: object, label: str) -> tuple[float, float]:
    """Validate finite serialized geometry without coercing external values."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise CubeGraphMutationError(f"Cube {label} is unavailable.")
    if len(value) < 2:
        raise CubeGraphMutationError(f"Cube {label} is incomplete.")
    first, second = value[0], value[1]
    if (
        isinstance(first, bool)
        or not isinstance(first, int | float)
        or isinstance(second, bool)
        or not isinstance(second, int | float)
        or not isfinite(float(first))
        or not isfinite(float(second))
    ):
        raise CubeGraphMutationError(f"Cube {label} must contain finite numbers.")
    return float(first), float(second)


def cube_position_order(
    workflow: Mapping[str, object], node_ids: Mapping[str, str]
) -> tuple[str, ...]:
    """Read all Cube positions independently of execution ports and links."""

    nodes = _cube_nodes(workflow, node_ids)
    return tuple(
        sorted(
            node_ids,
            key=lambda identity: (
                *read_cube_pair(nodes[identity].get("pos"), "position"),
                identity,
            ),
        )
    )


def reorder_cube_positions(
    workflow: dict[str, object],
    node_ids: Mapping[str, str],
    expected: Sequence[str],
    requested: Sequence[str],
) -> None:
    """Reflow a complete permutation using native swap gaps and vertical slots.

    Preserve the left anchor, authored inter-slot spacing, and vertical slots.
    Account for every reordered frame's width so unequal frames cannot overlap
    later Cubes. Mutate positions only; links and node payloads retain ownership.
    """

    current = cube_position_order(workflow, node_ids)
    if tuple(expected) != current:
        raise CubeGraphMutationError("Cube graph order changed before reorder.")
    if (
        len(requested) != len(current)
        or len(set(requested)) != len(requested)
        or set(requested) != set(current)
    ):
        raise CubeGraphMutationError("Cube reorder must be an exact Cube permutation.")
    if tuple(requested) == current:
        return
    nodes = _cube_nodes(workflow, node_ids)
    positions = [
        read_cube_pair(nodes[identity].get("pos"), "position") for identity in current
    ]
    widths = {
        identity: max(0.0, read_cube_pair(node.get("size"), "size")[0])
        for identity, node in nodes.items()
    }
    gaps = [
        max(MINIMUM_CUBE_GAP, positions[index + 1][0] - left - widths[identity])
        for index, (identity, (left, _)) in enumerate(
            zip(current[:-1], positions[:-1], strict=True)
        )
    ]
    left = positions[0][0]
    for index, identity in enumerate(requested):
        nodes[identity]["pos"] = [left, positions[index][1]]
        if index < len(gaps):
            left += widths[identity] + gaps[index]


def _cube_nodes(
    workflow: Mapping[str, object], node_ids: Mapping[str, str]
) -> dict[str, dict[str, object]]:
    """Resolve only validated Cube identities without interpreting ordinary nodes."""

    values = workflow.get("nodes")
    if not isinstance(values, list):
        raise CubeGraphMutationError("Workflow nodes are unavailable.")
    native: dict[str, dict[str, object]] = {
        str(value.get("id")): value for value in values if isinstance(value, dict)
    }
    if any(node_id not in native for node_id in node_ids.values()):
        raise CubeGraphMutationError("Cube graph node is unavailable.")
    return {identity: native[node_id] for identity, node_id in node_ids.items()}
