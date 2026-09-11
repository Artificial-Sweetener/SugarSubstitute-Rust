#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Match serialized native Cube boundaries without a LiteGraph runtime."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from math import inf

from .execution.models import CubeBoundaryEndpoint, ProximityConnection

_DEFAULT_PROXIMITY_RADIUS = 160.0


@dataclass(frozen=True)
class _Port:
    """Retain one free serialized Cube boundary in declaration order."""

    instance_id: str
    node_id: str
    slot: int
    name: str
    value_type: str


@dataclass(frozen=True)
class _CubeGeometry:
    """Retain graph-space rectangle data used by Cube proximity."""

    left: float
    top: float
    width: float
    height: float

    @property
    def right(self) -> float:
        """Return the connector-facing output edge."""

        return self.left + self.width

    @property
    def bottom(self) -> float:
        """Return the lower edge used for vertical adjacency."""

        return self.top + self.height


@dataclass(frozen=True, order=True)
class _Candidate:
    """Sort one compatible pair by the SugarCubes Cube-pair policy."""

    adjacency: float
    output_slot: int
    input_slot: int
    source_node_id: str
    target_node_id: str
    output: _Port
    input: _Port


class SerializedCubeProximityMatcher:
    """Project Cube-to-Cube proximity from durable native graph geometry."""

    def match(self, workflow: Mapping[str, object]) -> tuple[ProximityConnection, ...]:
        """Return deterministic free-port matches without frontend state."""

        nodes = _nodes_by_id(workflow)
        cube_nodes = {
            node_id: node
            for node_id, node in nodes.items()
            if _instance_id(node) is not None
        }
        occupied_outputs, occupied_inputs = _occupied_slots(workflow)
        candidates: list[_Candidate] = []
        for source_id, source in cube_nodes.items():
            source_geometry = _geometry(source)
            for target_id, target in cube_nodes.items():
                if source_id == target_id:
                    continue
                target_geometry = _geometry(target)
                adjacency = target_geometry.left - source_geometry.right
                if not 0 <= adjacency <= _DEFAULT_PROXIMITY_RADIUS:
                    continue
                if not _vertically_adjacent(
                    source_geometry, target_geometry, _DEFAULT_PROXIMITY_RADIUS
                ):
                    continue
                for output in _ports(
                    source, direction="outputs", occupied=occupied_outputs
                ):
                    for input_port in _ports(
                        target, direction="inputs", occupied=occupied_inputs
                    ):
                        if _types_compatible(output.value_type, input_port.value_type):
                            candidates.append(
                                _Candidate(
                                    adjacency,
                                    output.slot,
                                    input_port.slot,
                                    source_id,
                                    target_id,
                                    output,
                                    input_port,
                                )
                            )
        used_outputs: set[tuple[str, int]] = set()
        used_inputs: set[tuple[str, int]] = set()
        result: list[ProximityConnection] = []
        for candidate in sorted(candidates):
            output_key = (candidate.output.node_id, candidate.output.slot)
            input_key = (candidate.input.node_id, candidate.input.slot)
            if output_key in used_outputs or input_key in used_inputs:
                continue
            used_outputs.add(output_key)
            used_inputs.add(input_key)
            result.append(
                ProximityConnection(
                    source=CubeBoundaryEndpoint(
                        candidate.output.instance_id, candidate.output.name
                    ),
                    target=CubeBoundaryEndpoint(
                        candidate.input.instance_id, candidate.input.name
                    ),
                )
            )
        return tuple(result)

    def adjacent_instance_pairs(
        self, workflow: Mapping[str, object]
    ) -> tuple[tuple[str, str], ...]:
        """Return geometry-adjacent Cube pairs without requiring compatible ports."""

        nodes = _nodes_by_id(workflow)
        cube_nodes = {
            node_id: node
            for node_id, node in nodes.items()
            if _instance_id(node) is not None
        }
        pairs: list[tuple[str, str]] = []
        for source_id, source in cube_nodes.items():
            source_geometry = _geometry(source)
            source_instance_id = _instance_id(source)
            if source_instance_id is None:
                continue
            for target_id, target in cube_nodes.items():
                if source_id == target_id:
                    continue
                target_geometry = _geometry(target)
                adjacency = target_geometry.left - source_geometry.right
                target_instance_id = _instance_id(target)
                if (
                    target_instance_id is not None
                    and 0 <= adjacency <= _DEFAULT_PROXIMITY_RADIUS
                    and _vertically_adjacent(
                        source_geometry,
                        target_geometry,
                        _DEFAULT_PROXIMITY_RADIUS,
                    )
                ):
                    pairs.append((source_instance_id, target_instance_id))
        return tuple(sorted(set(pairs)))


def _nodes_by_id(workflow: Mapping[str, object]) -> dict[str, Mapping[str, object]]:
    """Index safe root nodes without interpreting ordinary-node payloads."""

    nodes = workflow.get("nodes")
    if not isinstance(nodes, Sequence) or isinstance(nodes, (str, bytes, bytearray)):
        return {}
    return {
        str(node["id"]): node
        for node in nodes
        if isinstance(node, Mapping)
        and "id" in node
        and not isinstance(node["id"], bool)
        and isinstance(node["id"], str | int)
    }


def _instance_id(node: Mapping[str, object]) -> str | None:
    """Read durable Cube identity without classifying ordinary nodes."""

    properties = node.get("properties")
    if not isinstance(properties, Mapping) or properties.get("sugarcubes_kind") not in {
        "cube",
        "cube_draft",
    }:
        return None
    metadata = properties.get("sugarcubes_cube")
    value = metadata.get("instance_id") if isinstance(metadata, Mapping) else None
    return value.strip() if isinstance(value, str) and value.strip() else None


def _ports(
    node: Mapping[str, object],
    *,
    direction: str,
    occupied: set[tuple[str, int]],
) -> tuple[_Port, ...]:
    """Read free native boundary slots in declaration order."""

    node_id = str(node.get("id"))
    instance_id = _instance_id(node)
    values = node.get(direction)
    if (
        instance_id is None
        or not isinstance(values, Sequence)
        or isinstance(values, (str, bytes, bytearray))
    ):
        return ()
    result: list[_Port] = []
    for slot, value in enumerate(values):
        if (node_id, slot) in occupied or not isinstance(value, Mapping):
            continue
        if direction == "inputs" and value.get("link") is not None:
            continue
        links = value.get("links")
        if (
            direction == "outputs"
            and isinstance(links, Sequence)
            and any(link is not None for link in links)
        ):
            continue
        name = value.get("name")
        value_type = value.get("type")
        if isinstance(name, str) and name.strip() and isinstance(value_type, str):
            result.append(
                _Port(instance_id, node_id, slot, name.strip(), value_type.strip())
            )
    return tuple(result)


def _occupied_slots(
    workflow: Mapping[str, object],
) -> tuple[set[tuple[str, int]], set[tuple[str, int]]]:
    """Index persisted links so stale slot decorations cannot duplicate edges."""

    outputs: set[tuple[str, int]] = set()
    inputs: set[tuple[str, int]] = set()
    links = workflow.get("links")
    if not isinstance(links, Sequence) or isinstance(links, (str, bytes, bytearray)):
        return outputs, inputs
    for value in links:
        parts = _link_parts(value)
        if parts is None:
            continue
        source_id, source_slot, target_id, target_slot = parts
        outputs.add((source_id, source_slot))
        inputs.add((target_id, target_slot))
    return outputs, inputs


def _link_parts(value: object) -> tuple[str, int, str, int] | None:
    """Normalize a supported root link for slot occupancy."""

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
        return None
    source_id, source_slot, target_id, target_slot = parts
    if (
        isinstance(source_id, bool)
        or not isinstance(source_id, str | int)
        or isinstance(source_slot, bool)
        or not isinstance(source_slot, int)
        or isinstance(target_id, bool)
        or not isinstance(target_id, str | int)
        or isinstance(target_slot, bool)
        or not isinstance(target_slot, int)
    ):
        return None
    return str(source_id), source_slot, str(target_id), target_slot


def _geometry(node: Mapping[str, object]) -> _CubeGeometry:
    """Read finite serialized rectangle data with non-matching fallbacks."""

    left, top = _pair(node.get("pos"), fallback=inf)
    width, height = _pair(node.get("size"), fallback=0.0)
    return _CubeGeometry(left, top, max(0.0, width), max(0.0, height))


def _pair(value: object, *, fallback: float) -> tuple[float, float]:
    """Normalize one serialized two-dimensional numeric value."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        return fallback, fallback
    numbers = [float(item) for item in value[:2] if isinstance(item, int | float)]
    return (numbers[0], numbers[1]) if len(numbers) == 2 else (fallback, fallback)


def _vertically_adjacent(
    source: _CubeGeometry, target: _CubeGeometry, margin: float
) -> bool:
    """Match the SugarCubes rectangle-overlap proximity contract."""

    return max(source.top, target.top) <= min(source.bottom, target.bottom) + margin


def _types_compatible(output_type: str, input_type: str) -> bool:
    """Accept exact, wildcard, or union-overlap Comfy boundary types."""

    outputs = _type_members(output_type)
    inputs = _type_members(input_type)
    return "*" in outputs or "*" in inputs or bool(outputs & inputs)


def _type_members(value: str) -> frozenset[str]:
    """Normalize a serialized Comfy type union for stable comparison."""

    return frozenset(
        member.strip().upper() for member in value.split(",") if member.strip()
    )


__all__ = ["SerializedCubeProximityMatcher"]
