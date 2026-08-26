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
"""Derive direct Cube-only topology from validated workflow boundaries."""

from __future__ import annotations

from collections import deque
from collections.abc import Mapping, Sequence

from .models import (
    ConnectionOrigin,
    CubeBoundaryConnection,
    CubeBoundaryEndpoint,
    CubeTopology,
    ProximityConnection,
)
from .limits import MAX_PROXIMITY_CONNECTIONS
from ..workflow import CanonicalWorkflow, CubeInstance
from .errors import CubeTopologyError


class CubeTopologyBuilder:
    """Own edge recognition, de-duplication, components, and cycle rejection."""

    def build(
        self,
        workflow: CanonicalWorkflow,
        proximity_connections: Sequence[ProximityConnection] = (),
    ) -> CubeTopology:
        """Build deterministic topology without inferring through loose nodes."""

        if len(proximity_connections) > MAX_PROXIMITY_CONNECTIONS:
            raise CubeTopologyError(
                "execution.topology.proximity_limit_exceeded",
                "Proximity connections exceed the execution safety limit of "
                f"{MAX_PROXIMITY_CONNECTIONS} entries.",
                (),
            )
        instances = {instance.instance_id: instance for instance in workflow.instances}
        by_node_id = {instance.node_id: instance for instance in workflow.instances}
        root_nodes = _root_nodes(workflow.payload)
        edges: dict[tuple[str, str, str, str], CubeBoundaryConnection] = {}
        for link_index, raw_link in enumerate(_workflow_links(workflow.payload)):
            parsed = _parse_link(raw_link)
            if parsed is None:
                continue
            origin_id, origin_slot, target_id, target_slot = parsed
            source_instance = by_node_id.get(origin_id)
            target_instance = by_node_id.get(target_id)
            if source_instance is None or target_instance is None:
                continue
            source_binding = _boundary_name(
                root_nodes[source_instance.node_id],
                "outputs",
                origin_slot,
                f"$.links[{link_index}].origin_slot",
            )
            target_binding = _boundary_name(
                root_nodes[target_instance.node_id],
                "inputs",
                target_slot,
                f"$.links[{link_index}].target_slot",
            )
            key = (
                source_instance.instance_id,
                source_binding,
                target_instance.instance_id,
                target_binding,
            )
            edges[key] = CubeBoundaryConnection(
                CubeBoundaryEndpoint(source_instance.instance_id, source_binding),
                CubeBoundaryEndpoint(target_instance.instance_id, target_binding),
                ConnectionOrigin.EXPLICIT,
            )
        for connection in proximity_connections:
            _validate_endpoint(connection.source, instances, root_nodes, "outputs")
            _validate_endpoint(connection.target, instances, root_nodes, "inputs")
            edge = CubeBoundaryConnection(
                connection.source,
                connection.target,
                ConnectionOrigin.PROXIMITY,
            )
            edges.setdefault(edge.semantic_key, edge)
        ordered_edges = tuple(
            sorted(edges.values(), key=lambda item: item.semantic_key)
        )
        _reject_cycles(instances, ordered_edges)
        return CubeTopology(
            instances=instances,
            edges=ordered_edges,
            components=_components(instances, ordered_edges),
            topological_order=_topological_order(instances, ordered_edges),
        )


def build_cube_topology(
    workflow: CanonicalWorkflow,
    proximity_connections: Sequence[ProximityConnection] = (),
) -> CubeTopology:
    """Build topology through the single authoritative stateless owner."""

    return CubeTopologyBuilder().build(workflow, proximity_connections)


def _workflow_links(payload: Mapping[str, object]) -> Sequence[object]:
    """Return root links without accepting text or malformed containers."""

    links = payload.get("links")
    if not isinstance(links, Sequence) or isinstance(links, (str, bytes, bytearray)):
        raise CubeTopologyError(
            "execution.topology.invalid_links",
            "Workflow links must be an array.",
            (),
        )
    return links


def _root_nodes(payload: Mapping[str, object]) -> dict[str, Mapping[str, object]]:
    """Index validated workflow nodes by normalized host identifier."""

    values = payload.get("nodes")
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes, bytearray)):
        raise CubeTopologyError(
            "execution.topology.invalid_nodes",
            "Workflow nodes must be an array.",
            (),
        )
    result: dict[str, Mapping[str, object]] = {}
    for value in values:
        if not isinstance(value, Mapping):
            continue
        node_id = value.get("id")
        if isinstance(node_id, bool) or not isinstance(node_id, (str, int)):
            continue
        result[str(node_id)] = value
    return result


def _parse_link(value: object) -> tuple[str, int, str, int] | None:
    """Read supported Comfy root-link forms and ignore malformed loose data."""

    if isinstance(value, Mapping):
        return _link_parts(
            value.get("origin_id"),
            value.get("origin_slot"),
            value.get("target_id"),
            value.get("target_slot"),
        )
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        if len(value) < 5:
            return None
        return _link_parts(value[1], value[2], value[3], value[4])
    return None


def _link_parts(
    origin_id: object,
    origin_slot: object,
    target_id: object,
    target_slot: object,
) -> tuple[str, int, str, int] | None:
    """Normalize one link only when both endpoints and slots are safe."""

    if (
        isinstance(origin_id, bool)
        or not isinstance(origin_id, (str, int))
        or isinstance(target_id, bool)
        or not isinstance(target_id, (str, int))
        or isinstance(origin_slot, bool)
        or not isinstance(origin_slot, int)
        or origin_slot < 0
        or isinstance(target_slot, bool)
        or not isinstance(target_slot, int)
        or target_slot < 0
    ):
        return None
    return str(origin_id), origin_slot, str(target_id), target_slot


def _boundary_name(
    definition: Mapping[str, object],
    direction: str,
    slot: int,
    path: str,
) -> str:
    """Resolve one unique native boundary position to its canonical name."""

    values = definition.get(direction)
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes, bytearray)):
        raise CubeTopologyError(
            "execution.topology.invalid_boundary",
            f"Cube definition {direction} must be an array for {path}.",
            (),
        )
    if slot >= len(values) or not isinstance(values[slot], Mapping):
        raise CubeTopologyError(
            "execution.topology.missing_boundary",
            f"Cube boundary slot {slot} does not exist for {path}.",
            (),
        )
    name = values[slot].get("name")
    if not isinstance(name, str) or not name.strip():
        raise CubeTopologyError(
            "execution.topology.invalid_boundary",
            f"Cube boundary at {path} has no stable name.",
            (),
        )
    return name.strip()


def _validate_endpoint(
    endpoint: CubeBoundaryEndpoint,
    instances: Mapping[str, CubeInstance],
    root_nodes: Mapping[str, Mapping[str, object]],
    direction: str,
) -> None:
    """Reject proximity observations that do not address a real Cube boundary."""

    instance = instances.get(endpoint.instance_id)
    if instance is None:
        raise CubeTopologyError(
            "execution.topology.unknown_instance",
            f"Proximity connection references unknown Cube '{endpoint.instance_id}'.",
            (endpoint.instance_id,),
        )
    node = root_nodes.get(instance.node_id, {})
    values = node.get(direction)
    entries = (
        values
        if isinstance(values, Sequence)
        and not isinstance(values, (str, bytes, bytearray))
        else ()
    )
    names = {
        name
        for value in entries
        if isinstance(value, Mapping)
        for name in (value.get("name"),)
        if isinstance(name, str)
    }
    if endpoint.binding not in names:
        raise CubeTopologyError(
            "execution.topology.unknown_boundary",
            f"Cube '{endpoint.instance_id}' has no {direction[:-1]} '{endpoint.binding}'.",
            (endpoint.instance_id,),
        )


def _reject_cycles(
    instances: Mapping[str, CubeInstance],
    edges: Sequence[CubeBoundaryConnection],
) -> None:
    """Reject directed Cube cycles before lowering or inheritance can guess."""

    children: dict[str, set[str]] = {instance_id: set() for instance_id in instances}
    indegree = {instance_id: 0 for instance_id in instances}
    for edge in edges:
        source = edge.source_instance_id
        target = edge.target_instance_id
        if target in children[source]:
            continue
        children[source].add(target)
        indegree[target] += 1
    ready = deque(sorted(key for key, degree in indegree.items() if degree == 0))
    visited = 0
    while ready:
        current = ready.popleft()
        visited += 1
        for child in sorted(children[current]):
            indegree[child] -= 1
            if indegree[child] == 0:
                ready.append(child)
    if visited == len(instances):
        return
    cycle_instances = tuple(
        sorted(key for key, degree in indegree.items() if degree > 0)
    )
    raise CubeTopologyError(
        "execution.topology.cycle",
        "Cube inheritance topology contains a directed cycle.",
        cycle_instances,
    )


def _topological_order(
    instances: Mapping[str, CubeInstance],
    edges: Sequence[CubeBoundaryConnection],
) -> tuple[str, ...]:
    """Return stable dependency order after cycle validation."""

    children: dict[str, set[str]] = {instance_id: set() for instance_id in instances}
    indegree = {instance_id: 0 for instance_id in instances}
    for edge in edges:
        source = edge.source_instance_id
        target = edge.target_instance_id
        if target in children[source]:
            continue
        children[source].add(target)
        indegree[target] += 1
    ready = sorted(key for key, degree in indegree.items() if degree == 0)
    result: list[str] = []
    while ready:
        current = ready.pop(0)
        result.append(current)
        for child in sorted(children[current]):
            indegree[child] -= 1
            if indegree[child] == 0:
                ready.append(child)
                ready.sort()
    return tuple(result)


def _components(
    instances: Mapping[str, CubeInstance],
    edges: Sequence[CubeBoundaryConnection],
) -> tuple[tuple[str, ...], ...]:
    """Return undirected Cube-only components with stable ordering."""

    neighbors: dict[str, set[str]] = {instance_id: set() for instance_id in instances}
    for edge in edges:
        source = edge.source_instance_id
        target = edge.target_instance_id
        neighbors[source].add(target)
        neighbors[target].add(source)
    remaining = set(instances)
    components: list[tuple[str, ...]] = []
    while remaining:
        start = min(remaining)
        queue = deque([start])
        member_ids: set[str] = set()
        while queue:
            current = queue.popleft()
            if current in member_ids:
                continue
            member_ids.add(current)
            queue.extend(sorted(neighbors[current] - member_ids))
        remaining -= member_ids
        components.append(tuple(sorted(member_ids)))
    return tuple(sorted(components, key=lambda item: item[0]))
