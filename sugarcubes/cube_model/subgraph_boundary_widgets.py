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
"""Identify widget fields supplied by a serialized subgraph input boundary."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any, TypeAlias, TypeGuard

GraphId: TypeAlias = str | int
SUBGRAPH_INPUT_NODE_ID = -10


def index_boundary_widget_names(
    subgraph: Mapping[str, Any],
) -> dict[GraphId, frozenset[str]]:
    """Index boundary-supplied widget names by their target node identity."""

    boundary_targets = _boundary_link_targets(subgraph.get("links"))
    nodes = subgraph.get("nodes")
    if not _is_sequence(nodes):
        return {}
    indexed: dict[GraphId, frozenset[str]] = {}
    for node in nodes:
        if not isinstance(node, Mapping):
            continue
        node_id = _read_graph_id(node.get("id"))
        inputs = node.get("inputs")
        if node_id is None or not _is_sequence(inputs):
            continue
        names = {
            name
            for entry in inputs
            if isinstance(entry, Mapping)
            if _input_targets_node_boundary(entry, node_id, boundary_targets)
            if (name := _widget_name(entry)) is not None
        }
        if names:
            indexed[node_id] = frozenset(names)
    return indexed


def index_boundary_widget_targets(
    subgraph: Mapping[str, Any],
) -> dict[str, tuple[GraphId, str]]:
    """Map each public boundary name to one exact inner widget identity."""

    links = _index_links(subgraph.get("links"))
    nodes = _index_nodes(subgraph.get("nodes"))
    result: dict[str, tuple[GraphId, str]] = {}
    boundaries = subgraph.get("inputs")
    if not _is_sequence(boundaries):
        return result
    for boundary in boundaries:
        if not isinstance(boundary, Mapping):
            continue
        boundary_name = _text(boundary.get("name"))
        link_ids = boundary.get("linkIds")
        targets: set[tuple[GraphId, str]] = set()
        if not boundary_name or not _is_sequence(link_ids):
            continue
        for raw_link_id in link_ids:
            link_id = _read_graph_id(raw_link_id)
            link = links.get(link_id) if link_id is not None else None
            if not link or _read_graph_id(link.get("origin_id")) != SUBGRAPH_INPUT_NODE_ID:
                continue
            node_id = _read_graph_id(link.get("target_id"))
            slot = link.get("target_slot")
            node = nodes.get(node_id) if node_id is not None else None
            inputs = node.get("inputs") if node is not None else None
            if (
                not isinstance(slot, int)
                or isinstance(slot, bool)
                or not _is_sequence(inputs)
                or slot < 0
                or slot >= len(inputs)
            ):
                continue
            entry = inputs[slot]
            name = _widget_name(entry) if isinstance(entry, Mapping) else None
            if node_id is not None and name:
                targets.add((node_id, name))
        if len(targets) == 1:
            result[boundary_name] = next(iter(targets))
    return result


def _index_links(value: object) -> dict[GraphId, Mapping[str, Any]]:
    """Index serialized links by their exact graph identity."""

    if not _is_sequence(value):
        return {}
    return {
        link_id: link
        for link in value
        if isinstance(link, Mapping)
        if (link_id := _read_graph_id(link.get("id"))) is not None
    }


def _index_nodes(value: object) -> dict[GraphId, Mapping[str, Any]]:
    """Index serialized nodes by their exact graph identity."""

    if not _is_sequence(value):
        return {}
    return {
        node_id: node
        for node in value
        if isinstance(node, Mapping)
        if (node_id := _read_graph_id(node.get("id"))) is not None
    }


def _text(value: object) -> str:
    """Read one nonempty boundary name."""

    return value.strip() if isinstance(value, str) else ""


def _boundary_link_targets(value: object) -> dict[GraphId, GraphId]:
    """Map subgraph-input link identities to their exact target nodes."""

    if not _is_sequence(value):
        return {}
    targets: dict[GraphId, GraphId] = {}
    for link in value:
        if not isinstance(link, Mapping):
            continue
        origin_id = _read_graph_id(link.get("origin_id"))
        link_id = _read_graph_id(link.get("id"))
        target_id = _read_graph_id(link.get("target_id"))
        if origin_id == SUBGRAPH_INPUT_NODE_ID and link_id is not None and target_id is not None:
            targets[link_id] = target_id
    return targets


def _input_targets_node_boundary(
    entry: Mapping[str, Any],
    node_id: GraphId,
    boundary_targets: Mapping[GraphId, GraphId],
) -> bool:
    """Return whether one widget input is linked from its subgraph boundary."""

    link_id = _read_graph_id(entry.get("link"))
    return link_id is not None and boundary_targets.get(link_id) == node_id


def _widget_name(entry: Mapping[str, Any]) -> str | None:
    """Read the stable widget name adjacent to a serialized input."""

    widget = entry.get("widget")
    if not isinstance(widget, Mapping):
        return None
    raw_name = widget.get("name", entry.get("name"))
    return raw_name.strip() if isinstance(raw_name, str) and raw_name.strip() else None


def _read_graph_id(value: object) -> GraphId | None:
    """Narrow one serialized graph identity without coercing unrelated values."""

    if isinstance(value, bool):
        return None
    return value if isinstance(value, str | int) else None


def _is_sequence(value: object) -> TypeGuard[Sequence[Any]]:
    """Return whether a value is a non-string sequence."""

    return isinstance(value, Sequence) and not isinstance(value, str | bytes)
