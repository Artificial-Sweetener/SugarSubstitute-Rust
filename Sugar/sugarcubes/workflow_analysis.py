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
#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Analyze serialized Cube graphs without importing Comfy frontend objects."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from math import inf, isfinite

from .execution import (
    ConnectionOrigin,
    CubeBoundaryConnection,
    ProximityConnection,
)
from .execution.limits import MAX_PROXIMITY_CONNECTIONS
from .execution.models import CubeBoundaryEndpoint
from .execution.topology import CubeTopologyBuilder
from .workflow import ComposedValueMaterializer, CubeInstance, read_canonical_workflow
from .workflow import WorkflowNormalizer
from .serialized_cube_proximity import SerializedCubeProximityMatcher


@dataclass(frozen=True)
class CubeGraphSegment:
    """Describe one directly connected Cube component and reorder policy."""

    instance_ids: tuple[str, ...]
    reorderable: bool
    boundary_node_ids: tuple[str, ...] = ()


@dataclass(frozen=True)
class CubeGraphAnalysis:
    """Return the complete Cube projection derived from one native workflow."""

    workflow_semantic_hash: str
    instances: tuple[CubeInstance, ...]
    edges: tuple[CubeBoundaryConnection, ...]
    proximity_connections: tuple[ProximityConnection, ...]
    segments: tuple[CubeGraphSegment, ...]
    normalized_workflow: Mapping[str, object]
    # The stack: every Cube in the order it runs.
    order: tuple[str, ...]


@dataclass(frozen=True)
class CubeStackView:
    """What the SugarCubes page knows about a workflow's stack and sends with it.

    The page's proximity matcher is the one that decides which Cubes link by proximity, and its
    Cube chain is the one stack order. A client that sends neither (the Python SugarSubstitute
    app) gets the backend's own matcher and order, kept for it and planned for removal.
    """

    proximity_connections: tuple[ProximityConnection, ...]
    order: tuple[str, ...]


class CubeGraphAnalysisService:
    """Own headless Cube proximity and stack-segment projection."""

    def __init__(self, normalizer: WorkflowNormalizer | None = None) -> None:
        """Bind optional exact-version normalization for persisted graphs."""

        self._normalizer = normalizer

    def analyze(
        self, workflow_value: object, stack: CubeStackView | None = None
    ) -> CubeGraphAnalysis:
        """Analyze one complete serialized workflow in a single pass.

        With `stack`, the page's proximity links and stack order are used as given.
        """

        normalized = (
            self._normalizer.normalize(workflow_value)
            if self._normalizer is not None
            else workflow_value
        )
        materialized = ComposedValueMaterializer().materialize(normalized)
        workflow = read_canonical_workflow(materialized)
        if stack is None:
            matcher = SerializedCubeProximityMatcher()
            proximity = matcher.match(workflow.payload)
            adjacent = matcher.adjacent_instance_pairs(workflow.payload)
        else:
            proximity = stack.proximity_connections
            adjacent = tuple(
                (connection.source.instance_id, connection.target.instance_id)
                for connection in proximity
            )
        topology = CubeTopologyBuilder().project(workflow, proximity)
        segments = _segments(workflow.payload, topology, adjacent)
        computed = tuple(
            identity for segment in segments for identity in segment.instance_ids
        )
        return CubeGraphAnalysis(
            workflow_semantic_hash=workflow.semantic_hash,
            instances=workflow.instances,
            edges=topology.edges,
            proximity_connections=proximity,
            segments=segments,
            normalized_workflow=workflow.payload,
            order=_given_order(stack, computed),
        )


def read_cube_stack_view(value: object) -> CubeStackView | None:
    """Read the stack a page sent with a workflow; None when it sent none."""

    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise ValueError("The Cube stack must be an object.")
    order = value.get("order")
    if not isinstance(order, list) or not all(
        isinstance(identity, str) and identity.strip() for identity in order
    ):
        raise ValueError("The Cube stack order must list Cube instance ids.")
    return CubeStackView(
        proximity_connections=read_proximity_connections(
            value.get("proximity_connections")
        ),
        order=tuple(identity.strip() for identity in order),
    )


def read_proximity_connections(value: object) -> tuple[ProximityConnection, ...]:
    """Read explicit Cube-to-Cube proximity link records."""

    if not isinstance(value, list):
        raise ValueError("'proximity_connections' must be an array")
    if len(value) > MAX_PROXIMITY_CONNECTIONS:
        raise ValueError(
            "Proximity connections exceed the execution safety limit of "
            f"{MAX_PROXIMITY_CONNECTIONS} entries."
        )
    result: list[ProximityConnection] = []
    for entry in value:
        if not isinstance(entry, Mapping):
            raise ValueError("Expected a JSON object")
        result.append(
            ProximityConnection(
                source=CubeBoundaryEndpoint(
                    _required_text(entry, "source_instance_id"),
                    _required_text(entry, "source_binding"),
                ),
                target=CubeBoundaryEndpoint(
                    _required_text(entry, "target_instance_id"),
                    _required_text(entry, "target_binding"),
                ),
            )
        )
    return tuple(result)


def _required_text(record: Mapping[object, object], name: str) -> str:
    """Read one non-empty endpoint identifier."""

    value = record.get(name)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"'{name}' must be a non-empty string")
    return value.strip()


def analyze_cube_graph(workflow_value: object) -> CubeGraphAnalysis:
    """Analyze a native workflow through SugarCubes' stateless owner."""

    return CubeGraphAnalysisService().analyze(workflow_value)


def _segments(
    workflow: Mapping[str, object],
    topology: object,
    adjacent_pairs: tuple[tuple[str, str], ...],
) -> tuple[CubeGraphSegment, ...]:
    """Project deterministic linear segments and their loose-node boundaries."""

    instances = getattr(topology, "instances")
    edges = getattr(topology, "edges")
    topological_order = getattr(topology, "topological_order")
    by_node_id = {
        instance.node_id: instance.instance_id for instance in instances.values()
    }
    boundary_nodes: dict[str, set[str]] = {
        instance_id: set() for instance_id in instances
    }
    for source_id, _source_slot, target_id, _target_slot in _parsed_links(workflow):
        source_instance = by_node_id.get(source_id)
        target_instance = by_node_id.get(target_id)
        if source_instance is not None and target_instance is None:
            boundary_nodes[source_instance].add(target_id)
        elif source_instance is None and target_instance is not None:
            boundary_nodes[target_instance].add(source_id)
    explicit_pairs = {
        (edge.source_instance_id, edge.target_instance_id)
        for edge in edges
        if edge.origin is ConnectionOrigin.EXPLICIT
    }
    components = _segment_components(
        tuple(instances),
        adjacent_pairs=adjacent_pairs,
        explicit_pairs=explicit_pairs,
    )
    result: list[CubeGraphSegment] = []
    ordered_components = sorted(
        components,
        key=lambda component: min(
            _instance_layout_key(instances[instance_id]) for instance_id in component
        ),
    )
    for component in ordered_components:
        members = set(component)
        ordered = tuple(item for item in topological_order if item in members)
        component_explicit_pairs = {
            pair for pair in explicit_pairs if pair[0] in members and pair[1] in members
        }
        if not component_explicit_pairs or len(ordered) != len(members):
            ordered = tuple(
                sorted(members, key=lambda item: _instance_layout_key(instances[item]))
            )
        component_boundary_nodes = {
            node_id
            for instance_id in members
            for node_id in boundary_nodes[instance_id]
        }
        result.append(
            CubeGraphSegment(
                instance_ids=ordered,
                reorderable=(
                    len(ordered) > 1
                    and not component_explicit_pairs
                    and not component_boundary_nodes
                ),
                boundary_node_ids=tuple(sorted(component_boundary_nodes)),
            )
        )
    return tuple(result)


def _given_order(
    stack: CubeStackView | None, computed: tuple[str, ...]
) -> tuple[str, ...]:
    """Use the page's stack order when it names exactly this workflow's Cubes."""

    if stack is None or sorted(stack.order) != sorted(computed):
        return computed
    return stack.order


def _segment_components(
    instance_ids: tuple[str, ...],
    *,
    adjacent_pairs: tuple[tuple[str, str], ...],
    explicit_pairs: set[tuple[str, str]],
) -> tuple[frozenset[str], ...]:
    """Group Cubes by visual adjacency or explicit graph ownership."""

    neighbors: dict[str, set[str]] = {
        instance_id: set() for instance_id in instance_ids
    }
    for source_id, target_id in (*adjacent_pairs, *explicit_pairs):
        if source_id in neighbors and target_id in neighbors:
            neighbors[source_id].add(target_id)
            neighbors[target_id].add(source_id)
    components: list[frozenset[str]] = []
    remaining = set(instance_ids)
    while remaining:
        pending = [next(iter(remaining))]
        component: set[str] = set()
        while pending:
            instance_id = pending.pop()
            if instance_id in component:
                continue
            component.add(instance_id)
            pending.extend(neighbors[instance_id] - component)
        remaining -= component
        components.append(frozenset(component))
    return tuple(components)


def _instance_layout_key(instance: object) -> tuple[float, float, str]:
    """Order disconnected Cube regions from native graph geometry only."""

    payload = getattr(instance, "payload", None)
    position = payload.get("pos") if isinstance(payload, Mapping) else None
    if (
        isinstance(position, Sequence)
        and not isinstance(position, (str, bytes, bytearray))
        and len(position) >= 2
    ):
        left, top = position[0], position[1]
        if (
            not isinstance(left, bool)
            and isinstance(left, int | float)
            and not isinstance(top, bool)
            and isinstance(top, int | float)
            and isfinite(float(left))
            and isfinite(float(top))
        ):
            return float(left), float(top), str(getattr(instance, "instance_id", ""))
    return inf, inf, str(getattr(instance, "instance_id", ""))


def _parsed_links(
    workflow: Mapping[str, object],
) -> tuple[tuple[str, int, str, int], ...]:
    """Read supported root-link shapes for occupancy and boundary analysis."""

    links = workflow.get("links")
    if not isinstance(links, Sequence) or isinstance(links, (str, bytes, bytearray)):
        return ()
    result: list[tuple[str, int, str, int]] = []
    for value in links:
        if isinstance(value, Mapping):
            parts = (
                value.get("origin_id"),
                value.get("origin_slot"),
                value.get("target_id"),
                value.get("target_slot"),
            )
        elif (
            isinstance(value, Sequence)
            and not isinstance(value, (str, bytes, bytearray))
            and len(value) >= 5
        ):
            parts = (value[1], value[2], value[3], value[4])
        else:
            continue
        source_id, source_slot, target_id, target_slot = parts
        if (
            not isinstance(source_id, bool)
            and isinstance(source_id, str | int)
            and isinstance(source_slot, int)
            and not isinstance(source_slot, bool)
            and not isinstance(target_id, bool)
            and isinstance(target_id, str | int)
            and isinstance(target_slot, int)
            and not isinstance(target_slot, bool)
        ):
            result.append((str(source_id), source_slot, str(target_id), target_slot))
    return tuple(result)


__all__ = [
    "CubeGraphAnalysis",
    "CubeGraphAnalysisService",
    "CubeGraphSegment",
    "CubeStackView",
    "read_cube_stack_view",
    "read_proximity_connections",
    "analyze_cube_graph",
]
