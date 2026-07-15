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
"""Graph analysis utilities for SugarCubes exporter."""

from __future__ import annotations

from collections import defaultdict, deque


from dataclasses import dataclass, field


from typing import (
    Any,
    Deque,
    Dict,
    Iterator,
    List,
    Mapping,
    MutableMapping,
    Optional,
    Sequence,
    Set,
    Tuple,
)

MARKER_CLASS_TYPES = {
    "SugarCubes.CubeInput": "input",
    "SugarCubes.CubeOutput": "output",
}


@dataclass(frozen=True)
class Edge:
    """Directed connection between nodes."""

    source: str

    source_slot: int

    target: str

    target_port: str


@dataclass
class GraphNode:
    """Normalized view of a ComfyUI node."""

    id: str

    class_type: str

    inputs: Dict[str, Any]

    meta: Dict[str, Any]

    data: Dict[str, Any]

    def widget(self, name: str, default: Optional[Any] = None) -> Any:
        """Return one normalized widget value from the node payload."""

        value = self.inputs.get(name, default)

        return value


@dataclass
class Graph:
    """Graph with adjacency lookups."""

    nodes: Dict[str, GraphNode]

    outgoing: Dict[str, List[Edge]]

    incoming: Dict[str, List[Edge]]

    def edges_from(self, node_id: str) -> Sequence[Edge]:
        """Return outgoing edges for the given node id."""

        return self.outgoing.get(node_id, [])

    def edges_to(self, node_id: str) -> Sequence[Edge]:
        """Return incoming edges for the given node id."""

        return self.incoming.get(node_id, [])

    def iter_edges(self) -> Iterator[Edge]:
        """Iterate every edge in the graph once."""

        for edges in self.outgoing.values():
            for edge in edges:
                yield edge


@dataclass
class CubeMarker:
    """Marker node metadata."""

    node_id: str

    cube_id: str

    default_alias: str

    kind: str


@dataclass
class CubeData:
    """Per-cube aggregation of markers and discovered nodes."""

    cube_id: str

    name: str

    name_from_lookup: bool = False

    inputs: List[CubeMarker] = field(default_factory=list)

    outputs: List[CubeMarker] = field(default_factory=list)

    subgraph_nodes: Set[str] = field(default_factory=set)

    def marker_ids(self) -> Set[str]:
        """Return the marker node ids owned by the cube."""

        ids: Set[str] = {m.node_id for m in self.inputs}

        ids.update(m.node_id for m in self.outputs)

        return ids


@dataclass
class CubeAnalysis:
    """Full analysis result for the exporter."""

    graph: Graph

    cubes: Dict[str, CubeData]

    markers_by_id: Dict[str, CubeMarker]

    membership: Dict[str, Set[str]]


def build_graph(
    raw: Mapping[str, Any], workflow: Optional[Mapping[str, Any]] = None
) -> Graph:
    """Convert `app.graphToPrompt().prompt` into a Graph, merging workflow nodes when needed."""

    if "prompt" in raw and isinstance(raw["prompt"], Mapping):
        payload = raw["prompt"]

    else:
        payload = raw

    if not isinstance(payload, Mapping):
        raise TypeError("Prompt payload must be a mapping of node id to node data")

    nodes: Dict[str, GraphNode] = {}

    outgoing: Dict[str, List[Edge]] = defaultdict(list)

    incoming: Dict[str, List[Edge]] = defaultdict(list)

    for node_id, node_data in payload.items():
        if not isinstance(node_id, str):
            raise TypeError("Prompt node ids must be strings")

        if not node_id.isdigit():
            # Non-numeric keys (e.g. 'workflow', 'output') are metadata in the modern ComfyUI prompt
            continue

        if not isinstance(node_data, Mapping):
            raise TypeError(f"Node '{node_id}' payload must be a mapping")

        class_type = node_data.get("class_type")

        if not isinstance(class_type, str):
            if node_id == "workflow":
                # ComfyUI (Sept 2025) wraps graph metadata under a workflow key
                continue
            raise TypeError(f"Node '{node_id}' is missing a string class_type")

        inputs = node_data.get("inputs", {})

        if not isinstance(inputs, Mapping):
            raise TypeError(f"Node '{node_id}' inputs must be a mapping")

        meta = node_data.get("_meta", {})

        if meta is None:
            meta = {}

        if not isinstance(meta, Mapping):
            raise TypeError(f"Node '{node_id}' _meta must be a mapping if present")

        normalized = GraphNode(
            id=str(node_id),
            class_type=class_type,
            inputs=dict(inputs),
            meta=dict(meta),
            data=dict(node_data),
        )

        nodes[str(node_id)] = normalized

    for node in nodes.values():
        for port, value in node.inputs.items():
            for source, slot in _iter_links(value):
                if source not in nodes:
                    continue

                edge = Edge(
                    source=source, source_slot=slot, target=node.id, target_port=port
                )

                outgoing[source].append(edge)

                incoming[node.id].append(edge)

    if workflow:
        _merge_workflow_nodes(nodes, workflow)

        outgoing.clear()

        incoming.clear()

        for node in nodes.values():
            for port, value in node.inputs.items():
                for source, slot in _iter_links(value):
                    if source not in nodes:
                        continue

                    edge = Edge(
                        source=source,
                        source_slot=slot,
                        target=node.id,
                        target_port=port,
                    )

                    outgoing[source].append(edge)

                    incoming[node.id].append(edge)

    return Graph(nodes=nodes, outgoing=dict(outgoing), incoming=dict(incoming))


def analyze_cubes(
    raw: Mapping[str, Any],
    workflow: Optional[Mapping[str, Any]] = None,
    default_alias_lookup: Optional[Mapping[str, str]] = None,
) -> CubeAnalysis:
    """Inspect the prompt and return cube analysis."""

    graph = build_graph(raw, workflow=workflow)

    cubes, markers_by_id = _collect_markers(
        graph, default_alias_lookup=default_alias_lookup
    )

    _compute_subgraphs(graph, cubes)

    membership = _build_membership(cubes)

    return CubeAnalysis(
        graph=graph,
        cubes=cubes,
        markers_by_id=markers_by_id,
        membership=membership,
    )


def _collect_markers(
    graph: Graph,
    *,
    default_alias_lookup: Optional[Mapping[str, str]] = None,
) -> Tuple[Dict[str, CubeData], Dict[str, CubeMarker]]:
    """Collect cube markers and group them under their owning cube ids."""

    cubes: Dict[str, CubeData] = {}
    markers_by_id: Dict[str, CubeMarker] = {}

    for node in graph.nodes.values():
        marker_kind = MARKER_CLASS_TYPES.get(node.class_type)
        if not marker_kind:
            continue

        default_alias = _read_default_alias(node)
        cube_id = _read_cube_id(node)
        if not default_alias:
            raise ValueError(f"Marker node '{node.id}' is missing default_alias")
        if not cube_id:
            raise ValueError(f"Marker node '{node.id}' is missing cube_id")

        marker = CubeMarker(
            node_id=node.id,
            cube_id=cube_id,
            default_alias=default_alias,
            kind=marker_kind,
        )

        if cube_id not in cubes:
            canonical_name = default_alias
            canonical_name_from_lookup = False
            if default_alias_lookup and cube_id in default_alias_lookup:
                resolved = default_alias_lookup.get(cube_id)
                if (
                    isinstance(resolved, str)
                    and resolved.strip()
                    and _should_use_lookup_default_alias(
                        default_alias,
                        cube_id,
                        resolved.strip(),
                    )
                ):
                    canonical_name = resolved.strip()
                    canonical_name_from_lookup = True
            cubes[cube_id] = CubeData(
                cube_id=cube_id,
                name=canonical_name,
                name_from_lookup=canonical_name_from_lookup,
            )

        cube = cubes[cube_id]
        if marker_kind == "input":
            cube.inputs.append(marker)
        elif marker_kind == "output":
            cube.outputs.append(marker)

        markers_by_id[node.id] = marker

    return cubes, markers_by_id


def _compute_subgraphs(graph: Graph, cubes: Dict[str, CubeData]) -> None:
    """Discover the executable node set that belongs to each cube."""

    for cube in cubes.values():
        forward = _forward_reachable(graph, cube)

        backward = _backward_reachable(graph, cube)

        has_sources = bool(cube.inputs)

        has_sinks = bool(cube.outputs)

        if has_sources and has_sinks:
            combined = forward & backward

            if not combined:
                combined = forward | backward

        elif has_sources:
            combined = set(forward)

        elif has_sinks:
            combined = set(backward)

        else:
            combined = set()

        if combined:
            combined = _expand_subgraph(graph, combined, cube.marker_ids())

        cube.subgraph_nodes = combined


def _forward_reachable(graph: Graph, cube: CubeData) -> Set[str]:
    """Traverse forward from input-like markers into the executable subgraph."""

    visited: Set[str] = set()

    queue: Deque[str] = deque()

    start_ids = [marker.node_id for marker in cube.inputs]

    for marker_id in start_ids:
        for edge in graph.edges_from(marker_id):
            target_node = graph.nodes.get(edge.target)

            if not target_node or _is_marker(target_node):
                continue

            if target_node.id not in visited:
                visited.add(target_node.id)

                queue.append(target_node.id)

    while queue:
        current = queue.popleft()

        for edge in graph.edges_from(current):
            target_node = graph.nodes.get(edge.target)

            if not target_node:
                continue

            if _is_marker(target_node):
                continue

            if target_node.id not in visited:
                visited.add(target_node.id)

                queue.append(target_node.id)

    return visited


def _backward_reachable(graph: Graph, cube: CubeData) -> Set[str]:
    """Traverse backward from output markers into the executable subgraph."""

    visited: Set[str] = set()

    queue: Deque[str] = deque()

    for marker in cube.outputs:
        for edge in graph.edges_to(marker.node_id):
            source_node = graph.nodes.get(edge.source)

            if not source_node or _is_marker(source_node):
                continue

            if source_node.id not in visited:
                visited.add(source_node.id)

                queue.append(source_node.id)

    while queue:
        current = queue.popleft()

        for edge in graph.edges_to(current):
            source_node = graph.nodes.get(edge.source)

            if not source_node:
                continue

            if _is_marker(source_node):
                continue

            if source_node.id not in visited:
                visited.add(source_node.id)

                queue.append(source_node.id)

    return visited


def _build_membership(cubes: Mapping[str, CubeData]) -> Dict[str, Set[str]]:
    """Index each node id by the cube ids that claim it."""

    membership: Dict[str, Set[str]] = defaultdict(set)

    for cube_id, cube in cubes.items():
        for node_id in cube.subgraph_nodes:
            membership[node_id].add(cube_id)

    return dict(membership)


def _expand_subgraph(graph: Graph, nodes: Set[str], marker_ids: Set[str]) -> Set[str]:
    """Close gaps around the discovered subgraph without crossing marker nodes."""

    if not nodes:
        return set()

    result = set(nodes)

    queue: Deque[str] = deque(nodes)

    while queue:
        current = queue.popleft()

        for edge in graph.edges_from(current):
            target_id = edge.target

            if target_id in marker_ids:
                continue

            target = graph.nodes.get(target_id)

            if not target or _is_marker(target):
                continue

            if target_id not in result:
                result.add(target_id)

                queue.append(target_id)

        for edge in graph.edges_to(current):
            source_id = edge.source

            if source_id in marker_ids:
                continue

            source = graph.nodes.get(source_id)

            if not source or _is_marker(source):
                continue

            if source_id not in result:
                result.add(source_id)

                queue.append(source_id)

    return result


def _read_default_alias(node: GraphNode) -> str:
    """Read the trimmed cube name from a marker node."""

    value = node.widget("default_alias", "")

    if isinstance(value, str):
        return value.strip()

    return str(value).strip() if value is not None else ""


def _read_cube_id(node: GraphNode) -> str:
    """Read the trimmed cube id from a marker node."""

    value = node.widget("cube_id", "")

    if isinstance(value, str):
        return value.strip()

    return str(value).strip() if value is not None else ""


def _should_use_lookup_default_alias(
    live_name: str,
    cube_id: str,
    lookup_name: str,
) -> bool:
    """Return whether saved-name lookup should replace the live graph name."""

    normalized_live = live_name.strip()
    normalized_lookup = lookup_name.strip()
    if not normalized_lookup or normalized_lookup == normalized_live:
        return False
    if not normalized_live or normalized_live == cube_id.strip():
        return True
    return False


def _is_marker(node: GraphNode) -> bool:
    """Return whether the node is one of the SugarCubes marker classes."""

    if node.class_type in MARKER_CLASS_TYPES:
        return True
    return False


def _iter_links(value: Any) -> Iterator[Tuple[str, int]]:
    """Yield every serialized node link embedded inside an input value."""

    if isinstance(value, list):
        if len(value) == 2 and _looks_like_link(value):
            yield str(value[0]), int(value[1])
        else:
            for item in value:
                yield from _iter_links(item)


def _looks_like_link(value: Sequence[Any]) -> bool:
    """Return whether the value matches ComfyUI's `[node_id, slot]` link shape."""

    if len(value) != 2:
        return False
    node_id, slot = value
    if not isinstance(node_id, (str, int)):
        return False
    if not isinstance(slot, int):
        return False
    return True


def _merge_workflow_nodes(
    nodes: MutableMapping[str, GraphNode], workflow: Mapping[str, Any]
) -> None:
    """Merge workflow-only node metadata into the prompt graph view."""

    workflow_nodes = workflow.get("nodes")
    workflow_links = workflow.get("links")
    workflow_defs = workflow.get("definitions")

    if not isinstance(workflow_nodes, Sequence) or not isinstance(
        workflow_links, Sequence
    ):
        return

    link_lookup: Dict[int, Tuple[str, int]] = {}
    for link in workflow_links:
        if not isinstance(link, Sequence) or len(link) < 5:
            continue
        link_id = link[0]
        origin_id = link[1]
        origin_slot = link[2]
        if (
            isinstance(link_id, int)
            and isinstance(origin_id, (int, str))
            and isinstance(origin_slot, int)
        ):
            link_lookup[link_id] = (str(origin_id), origin_slot)

    subgraph_titles: Dict[str, str] = {}
    if isinstance(workflow_defs, Mapping):
        subgraphs = workflow_defs.get("subgraphs")
        if isinstance(subgraphs, Sequence):
            for entry in subgraphs:
                if not isinstance(entry, Mapping):
                    continue
                sub_id = entry.get("id")
                name = entry.get("name")
                if isinstance(sub_id, str) and isinstance(name, str) and name.strip():
                    subgraph_titles[sub_id] = name.strip()

    for raw_node in workflow_nodes:
        if not isinstance(raw_node, Mapping):
            continue
        node_id = raw_node.get("id")
        if node_id is None:
            continue
        node_id_str = str(node_id)
        existing = nodes.get(node_id_str)
        class_type = raw_node.get("type")
        if not isinstance(class_type, str):
            continue

        inputs_payload: Dict[str, Any] = {}
        raw_inputs = raw_node.get("inputs")
        if isinstance(raw_inputs, Sequence):
            for raw_input in raw_inputs:
                if not isinstance(raw_input, Mapping):
                    continue
                name = raw_input.get("name")
                if not isinstance(name, str) or not name.strip():
                    continue
                link_id = raw_input.get("link")
                if isinstance(link_id, int) and link_id in link_lookup:
                    origin_id, origin_slot = link_lookup[link_id]
                    inputs_payload[name] = [origin_id, origin_slot]

        meta: Dict[str, Any] = {}
        title = raw_node.get("title")
        if isinstance(title, str) and title.strip():
            meta["title"] = title.strip()
        elif class_type in subgraph_titles:
            meta["title"] = subgraph_titles[class_type]

        if existing:
            if inputs_payload:
                existing.inputs.update(inputs_payload)
                existing.data["inputs"] = dict(existing.inputs)
            if meta.get("title") and not existing.meta.get("title"):
                existing.meta["title"] = meta["title"]
                existing.data["_meta"] = dict(existing.meta)
            continue

        node_data: Dict[str, Any] = {
            "class_type": class_type,
            "inputs": dict(inputs_payload),
            "_meta": dict(meta),
        }

        nodes[node_id_str] = GraphNode(
            id=node_id_str,
            class_type=class_type,
            inputs=dict(inputs_payload),
            meta=meta,
            data=node_data,
        )
