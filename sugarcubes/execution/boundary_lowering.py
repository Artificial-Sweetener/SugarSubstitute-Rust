#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Project Cube boundaries and wire accepted Cube topology edges."""

from __future__ import annotations

from typing import Mapping, MutableMapping

from .errors import CubeLoweringError
from .models import BoundaryBinding, CubeBoundaryConnection, CubeTopology

_DISABLED_MODE = 2
_BYPASS_MODE = 4


def apply_topology_edges(
    prompt: MutableMapping[str, dict[str, object]],
    bindings: tuple[BoundaryBinding, ...],
    topology: CubeTopology,
) -> None:
    """Link lowered public endpoints exactly once per semantic target binding."""

    output_index: dict[tuple[str, str], BoundaryBinding] = {}
    input_index: dict[tuple[str, str], list[BoundaryBinding]] = {}
    for binding in bindings:
        key = (binding.instance_id, binding.binding)
        if binding.direction == "output":
            output_index[key] = binding
        else:
            input_index.setdefault(key, []).append(binding)
    edges_by_target: dict[tuple[str, str], list[CubeBoundaryConnection]] = {}
    for edge in topology.edges:
        edges_by_target.setdefault(
            (edge.target_instance_id, edge.target_binding), []
        ).append(edge)
    for target_key in sorted(edges_by_target):
        target_instance_id, target_binding = target_key
        if topology.instances[target_instance_id].execution_mode in {
            _DISABLED_MODE,
            _BYPASS_MODE,
        }:
            continue
        edges = edges_by_target[target_key]
        if len(edges) > 1:
            raise CubeLoweringError(
                "execution.lowering.multiple_boundary_sources",
                f"Cube input '{target_binding}' on '{target_instance_id}' has multiple sources.",
            )
        targets = input_index.get(target_key, [])
        source = _resolve_source(
            edges[0],
            prompt=prompt,
            output_index=output_index,
            input_index=input_index,
            topology=topology,
            seen=set(),
        )
        if source is None or not targets:
            continue
        for target in targets:
            wire_boundary_target(prompt, source, target)


def omit_unconnected_boundary_markers(
    prompt: MutableMapping[str, dict[str, object]],
) -> None:
    """Omit portable boundary placeholders that received no workflow edge.

    A connected boundary marker is replaced by :func:`apply_topology_edges`.
    Any marker still present afterward represents an unconnected public input;
    omitting that input lets Comfy apply the node's optional/default semantics
    instead of interpreting ``@binding`` as an execution-node identifier.
    """

    marker_owner = "@binding"
    for node in prompt.values():
        inputs = node.get("inputs")
        if not isinstance(inputs, MutableMapping):
            continue
        unconnected = [
            name
            for name, value in inputs.items()
            if isinstance(value, list)
            and len(value) == 2
            and value[0] == marker_owner
            and isinstance(value[1], str)
        ]
        for name in unconnected:
            del inputs[name]


def _resolve_source(
    edge: CubeBoundaryConnection,
    *,
    prompt: Mapping[str, Mapping[str, object]],
    output_index: Mapping[tuple[str, str], BoundaryBinding],
    input_index: Mapping[tuple[str, str], list[BoundaryBinding]],
    topology: CubeTopology,
    seen: set[tuple[str, str]],
) -> BoundaryBinding | None:
    """Resolve one source through outer bypass Cubes with type-safe fan-in."""

    source_key = (edge.source_instance_id, edge.source_binding)
    if source_key in seen:
        raise CubeLoweringError(
            "execution.lowering.outer_bypass_cycle",
            f"Outer Cube bypass resolution cycles at '{edge.source_instance_id}'.",
        )
    source = output_index.get(source_key)
    if source is None:
        return None
    mode = topology.instances[edge.source_instance_id].execution_mode
    if mode == _DISABLED_MODE:
        return None
    if mode != _BYPASS_MODE:
        return source if source.node_id in prompt else None
    next_seen = {*seen, source_key}
    candidates: list[BoundaryBinding] = []
    for incoming in topology.edges:
        if incoming.target_instance_id != edge.source_instance_id:
            continue
        target_bindings = input_index.get(
            (incoming.target_instance_id, incoming.target_binding), []
        )
        if not any(
            _types_compatible(source.value_type, target.value_type)
            for target in target_bindings
        ):
            continue
        resolved = _resolve_source(
            incoming,
            prompt=prompt,
            output_index=output_index,
            input_index=input_index,
            topology=topology,
            seen=next_seen,
        )
        if resolved is not None:
            candidates.append(resolved)
    unique = {
        (candidate.node_id, candidate.output_index): candidate
        for candidate in candidates
    }
    if len(unique) > 1:
        raise CubeLoweringError(
            "execution.lowering.ambiguous_outer_bypass",
            f"Bypassed Cube '{edge.source_instance_id}' has multiple compatible sources for '{edge.source_binding}'.",
        )
    return next(iter(unique.values()), None)


def _types_compatible(left: str, right: str) -> bool:
    """Treat explicit equal types and host wildcards as bypass-compatible."""

    normalized_left = left.strip().upper()
    normalized_right = right.strip().upper()
    return (
        not normalized_left
        or not normalized_right
        or "*" in {normalized_left, normalized_right}
        or normalized_left == normalized_right
    )


def wire_boundary_target(
    prompt: MutableMapping[str, dict[str, object]],
    source: BoundaryBinding,
    target: BoundaryBinding,
) -> None:
    """Write one validated boundary link into a derived prompt node."""

    if target.input_name is None or source.output_index is None:
        return
    node = prompt.get(target.node_id)
    if node is None:
        raise CubeLoweringError(
            "execution.lowering.missing_boundary_node",
            f"Cube boundary target node '{target.node_id}' was removed during lowering.",
        )
    inputs = node.get("inputs")
    if not isinstance(inputs, MutableMapping):
        raise CubeLoweringError(
            "execution.lowering.invalid_prompt_inputs",
            f"Lowered node '{target.node_id}' has no mutable input object.",
        )
    inputs[target.input_name] = [source.node_id, source.output_index]
