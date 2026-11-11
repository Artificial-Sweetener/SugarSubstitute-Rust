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
"""Serialize analyzed Cube boundary markers into canonical bindings."""

from __future__ import annotations

from typing import Any, Collection, Mapping, Sequence

from .graph import CubeData, CubeMarker, Edge, Graph
from .identifiers import sanitize_identifier
from .ordering import natural_node_key
from .versioning import resolve_input_type, resolve_output_type_by_slot


def build_input_bindings(
    cube: CubeData,
    graph: Graph,
    symbols: Mapping[str, str],
    definitions: Mapping[str, Any],
) -> tuple[dict[str, Any], dict[str, str], list[str]]:
    """Build canonical input bindings from analyzed Cube input markers."""

    inputs: dict[str, Any] = {}
    alias_lookup: dict[str, str] = {}
    warnings: list[str] = []
    counters: dict[tuple[str, str], int] = {}
    for marker in sorted(
        cube.inputs, key=lambda entry: natural_node_key(entry.node_id)
    ):
        edges = _downstream_edges(marker, graph, cube.subgraph_nodes)
        connections = [[symbols[edge.target], edge.target_port] for edge in edges]
        binding_type = _resolve_input_binding_type(edges, graph, definitions)
        alias = _make_binding_key("input", binding_type, counters)
        alias_lookup[marker.node_id] = alias
        inputs[alias] = {"kind": "input", "targets": connections}
        if not connections:
            warnings.append(
                f"CubeInput '{marker.node_id}' has no downstream connections"
            )
    return inputs, alias_lookup, warnings


def build_output_bindings(
    cube: CubeData,
    graph: Graph,
    symbols: Mapping[str, str],
    definitions: Mapping[str, Any],
) -> tuple[dict[str, Any], dict[str, str], list[str]]:
    """Build canonical output bindings with exact source-node slots."""

    outputs: dict[str, Any] = {}
    alias_lookup: dict[str, str] = {}
    warnings: list[str] = []
    counters: dict[tuple[str, str], int] = {}
    for marker in sorted(
        cube.outputs, key=lambda entry: natural_node_key(entry.node_id)
    ):
        upstream = _upstream_edges(marker, graph, cube.subgraph_nodes)
        if not upstream:
            warnings.append(f"CubeOutput '{marker.node_id}' has no upstream source")
            continue
        if len(upstream) > 1:
            warnings.append(
                f"CubeOutput '{marker.node_id}' has multiple upstream sources; taking the first"
            )
        edge = upstream[0]
        binding_type = _resolve_output_binding_type(edge, graph, definitions)
        alias = _make_binding_key("output", binding_type, counters)
        outputs[alias] = [symbols[edge.source], edge.source_slot]
        alias_lookup[marker.node_id] = alias
    return outputs, alias_lookup, warnings


def _downstream_edges(
    marker: CubeMarker, graph: Graph, subgraph: Collection[str]
) -> list[Edge]:
    """Collect edges from an input marker into executable Cube nodes."""

    subgraph_set = set(subgraph)
    edges = [
        edge for edge in graph.edges_from(marker.node_id) if edge.target in subgraph_set
    ]
    edges.sort(
        key=lambda edge: (natural_node_key(edge.target), str(edge.target_port or ""))
    )
    return edges


def _upstream_edges(
    marker: CubeMarker, graph: Graph, subgraph: Collection[str]
) -> list[Edge]:
    """Collect edges from executable Cube nodes into an output marker."""

    subgraph_set = set(subgraph)
    edges = [
        edge for edge in graph.edges_to(marker.node_id) if edge.source in subgraph_set
    ]
    edges.sort(
        key=lambda edge: (natural_node_key(edge.source), int(edge.source_slot or 0))
    )
    return edges


def _resolve_input_binding_type(
    edges: Sequence[Edge], graph: Graph, definitions: Mapping[str, Any]
) -> str:
    """Resolve the normalized type label for one canonical input binding."""

    if not edges:
        return "value"
    edge = edges[0]
    node = graph.nodes.get(edge.target)
    if not node:
        return "value"
    resolved = resolve_input_type(definitions, node.class_type, edge.target_port)
    return sanitize_identifier(resolved or "value")


def _resolve_output_binding_type(
    edge: Edge, graph: Graph, definitions: Mapping[str, Any]
) -> str:
    """Resolve the normalized type label for one canonical output binding."""

    node = graph.nodes.get(edge.source)
    if not node:
        return "value"
    resolved = resolve_output_type_by_slot(
        definitions, node.class_type, edge.source_slot
    )
    return sanitize_identifier(resolved or "value")


def _make_binding_key(
    direction: str,
    binding_type: str,
    counters: dict[tuple[str, str], int],
) -> str:
    """Build a stable unique alias for one canonical boundary binding."""

    base = sanitize_identifier(binding_type) or "value"
    key = (direction, base)
    count = counters.get(key, 0) + 1
    counters[key] = count
    suffix = "" if count == 1 else str(count)
    return f"{direction}.{base}{suffix}"
