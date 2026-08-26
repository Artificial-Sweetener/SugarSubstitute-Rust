#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Resolve MODEL, CLIP, and VAE inheritance through directed Cube topology."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping

from .errors import CubeInheritanceError
from .models import (
    CubeTopology,
    InheritanceResult,
    InheritanceSlot,
    InheritedBinding,
    LoweringResult,
)
from .inheritance_prompt import connection as _connection
from .inheritance_prompt import copy_prompt as _copy_prompt
from .inheritance_prompt import mutable_inputs as _mutable_inputs
from .inheritance_prompt import selector_input as _selector_input
from .resource_types import input_slot as _input_slot
from .resource_types import normalize_slot as _normalize_slot
from .resource_types import output_slot as _output_slot
from .resource_types import output_slots as _output_slots

_EMPTY_VALUES: tuple[object, ...] = (None, "", [], {})
_SLOT_ORDER: dict[InheritanceSlot, int] = {"model": 0, "clip": 1, "vae": 2}


@dataclass(frozen=True, order=True)
class _Provider:
    """Identify one origin resource output in the lowered prompt."""

    instance_id: str
    node_id: str
    output_index: int
    slot: InheritanceSlot


class CubeInheritanceResolver:
    """Own fail-closed resource inheritance over Cube-only direct edges."""

    def resolve(
        self, lowered: LoweringResult, topology: CubeTopology
    ) -> InheritanceResult:
        """Fill only unresolved inheritable inputs in a derived prompt copy."""

        prompt = _copy_prompt(lowered.prompt)
        all_origins = _all_origin_providers(prompt, lowered)
        live_origins = _live_origin_providers(prompt, lowered)
        reverse_edges = _reverse_edges(topology)
        inherited: list[InheritedBinding] = []
        unresolved = _unresolved_inputs(prompt, lowered)
        for target_instance_id, target_node_id, input_name, slot in unresolved:
            inputs = _mutable_inputs(prompt[target_node_id], target_node_id)
            if inputs.get(input_name) not in _EMPTY_VALUES:
                continue
            local = tuple(
                provider
                for provider in all_origins
                if provider.instance_id == target_instance_id and provider.slot == slot
            )
            provider = _unique_provider(
                local,
                target_instance_id=target_instance_id,
                slot=slot,
                scope="local",
            )
            distance = 0
            if provider is None:
                candidates, distance = _nearest_upstream_providers(
                    target_instance_id,
                    slot,
                    reverse_edges,
                    live_origins,
                )
                provider = _unique_provider(
                    candidates,
                    target_instance_id=target_instance_id,
                    slot=slot,
                    scope="upstream",
                )
            if provider is None:
                continue
            inputs[input_name] = [provider.node_id, provider.output_index]
            inherited.append(
                InheritedBinding(
                    target_instance_id=target_instance_id,
                    target_node_id=target_node_id,
                    input_name=input_name,
                    slot=slot,
                    source_instance_id=provider.instance_id,
                    source_node_id=provider.node_id,
                    output_index=provider.output_index,
                    distance=distance,
                )
            )
        return InheritanceResult(
            prompt=prompt,
            node_owners=lowered.node_owners,
            boundary_bindings=lowered.boundary_bindings,
            node_definitions=lowered.node_definitions,
            inherited_bindings=tuple(sorted(inherited, key=_inherited_key)),
        )


def _all_origin_providers(
    prompt: Mapping[str, Mapping[str, object]],
    lowered: LoweringResult,
) -> tuple[_Provider, ...]:
    """Return every active local origin regardless of current output usage."""

    providers: list[_Provider] = []
    for node_id in sorted(prompt):
        owner = lowered.node_owners[node_id]
        if owner.instance_id is None:
            continue
        for output_index, output_type in enumerate(
            _output_slots(lowered.node_definitions.get(node_id, {}))
        ):
            slot = _normalize_slot(output_type)
            if slot is not None and _is_origin(
                node_id, output_index, slot, prompt, lowered
            ):
                providers.append(
                    _Provider(owner.instance_id, node_id, output_index, slot)
                )
    return tuple(providers)


def _live_origin_providers(
    prompt: Mapping[str, Mapping[str, object]],
    lowered: LoweringResult,
) -> tuple[_Provider, ...]:
    """Trace consumed resource links through transparent selector nodes."""

    providers: set[_Provider] = set()
    transparent = _transparent_nodes(prompt, lowered)
    for target_node_id in sorted(prompt):
        if target_node_id in transparent:
            continue
        target = prompt[target_node_id]
        inputs = target.get("inputs")
        if not isinstance(inputs, Mapping):
            continue
        for input_name, value in inputs.items():
            slot = _input_slot(target_node_id, str(input_name), lowered)
            connection = _connection(value)
            if slot is None or connection is None:
                continue
            provider = _resolve_provider_source(
                connection[0],
                connection[1],
                slot,
                prompt,
                lowered,
                seen=set(),
            )
            if provider is not None:
                providers.add(provider)
    return tuple(sorted(providers))


def _resolve_provider_source(
    node_id: str,
    output_index: int,
    slot: InheritanceSlot,
    prompt: Mapping[str, Mapping[str, object]],
    lowered: LoweringResult,
    *,
    seen: set[tuple[str, int]],
) -> _Provider | None:
    """Trace one transparent output to a unique selected origin provider."""

    key = (node_id, output_index)
    if key in seen or node_id not in prompt:
        return None
    seen.add(key)
    owner = lowered.node_owners.get(node_id)
    if owner is None or owner.instance_id is None:
        return None
    output_slot = _output_slot(lowered.node_definitions.get(node_id, {}), output_index)
    if output_slot == slot and _is_origin(node_id, output_index, slot, prompt, lowered):
        return _Provider(owner.instance_id, node_id, output_index, slot)
    if output_slot not in {None, "*"}:
        return None
    node_inputs = prompt[node_id].get("inputs")
    if not isinstance(node_inputs, Mapping):
        return None
    candidates: list[_Provider] = []
    linked_names: list[str] = []
    for input_name, value in node_inputs.items():
        connection = _connection(value)
        if connection is None:
            continue
        linked_names.append(str(input_name))
        provider = _resolve_provider_source(
            connection[0],
            connection[1],
            slot,
            prompt,
            lowered,
            seen=set(seen),
        )
        if provider is not None:
            candidates.append(provider)
    if (
        candidates
        and linked_names
        and all(_selector_input(name) for name in linked_names)
    ):
        return candidates[0]
    unique = tuple(sorted(set(candidates)))
    return unique[0] if len(unique) == 1 else None


def _is_origin(
    node_id: str,
    output_index: int,
    slot: InheritanceSlot,
    prompt: Mapping[str, Mapping[str, object]],
    lowered: LoweringResult,
) -> bool:
    """Exclude wrapper gaps and derived resource transforms from origins."""

    node = prompt[node_id]
    inputs = node.get("inputs")
    if not isinstance(inputs, Mapping):
        return True
    required_source_names = (slot, f"{slot}_in")
    required_values = [inputs[name] for name in required_source_names if name in inputs]
    if required_values and not any(
        value not in _EMPTY_VALUES for value in required_values
    ):
        return False
    for input_name, value in inputs.items():
        if _connection(value) is None:
            continue
        if _input_slot(node_id, str(input_name), lowered) is not None:
            return False
    return _output_slot(lowered.node_definitions.get(node_id, {}), output_index) == slot


def _unresolved_inputs(
    prompt: Mapping[str, Mapping[str, object]],
    lowered: LoweringResult,
) -> list[tuple[str, str, str, InheritanceSlot]]:
    """Return missing Cube-owned resource inputs in semantic slot order."""

    result: list[tuple[str, str, str, InheritanceSlot]] = []
    for node_id in sorted(prompt):
        owner = lowered.node_owners[node_id]
        if owner.instance_id is None:
            continue
        inputs = prompt[node_id].get("inputs")
        if not isinstance(inputs, Mapping):
            continue
        for input_name, value in inputs.items():
            if value not in _EMPTY_VALUES:
                continue
            slot = _input_slot(node_id, str(input_name), lowered)
            if slot is not None:
                result.append((owner.instance_id, node_id, str(input_name), slot))
    result.sort(key=lambda item: (_SLOT_ORDER[item[3]], item[0], item[1], item[2]))
    return result


def _nearest_upstream_providers(
    target_instance_id: str,
    slot: InheritanceSlot,
    reverse_edges: Mapping[str, tuple[str, ...]],
    live_origins: tuple[_Provider, ...],
) -> tuple[tuple[_Provider, ...], int]:
    """Return providers at the first directed ancestor distance containing any."""

    by_instance: dict[str, list[_Provider]] = {}
    for provider in live_origins:
        if provider.slot == slot:
            by_instance.setdefault(provider.instance_id, []).append(provider)
    visited = {target_instance_id}
    frontier = tuple(reverse_edges.get(target_instance_id, ()))
    distance = 1
    while frontier:
        candidates = tuple(
            sorted(
                provider
                for instance_id in frontier
                for provider in by_instance.get(instance_id, ())
            )
        )
        if candidates:
            return candidates, distance
        visited.update(frontier)
        frontier = tuple(
            sorted(
                {
                    predecessor
                    for instance_id in frontier
                    for predecessor in reverse_edges.get(instance_id, ())
                    if predecessor not in visited
                }
            )
        )
        distance += 1
    return (), 0


def _unique_provider(
    providers: tuple[_Provider, ...],
    *,
    target_instance_id: str,
    slot: InheritanceSlot,
    scope: str,
) -> _Provider | None:
    """Return one provider or reject ambiguity without incidental ordering."""

    unique = tuple(sorted(set(providers)))
    if len(unique) <= 1:
        return unique[0] if unique else None
    provider_instances = tuple(sorted({provider.instance_id for provider in unique}))
    raise CubeInheritanceError(
        "execution.inheritance.ambiguous_provider",
        f"Cube '{target_instance_id}' has multiple equally eligible {scope} {slot.upper()} providers: "
        f"{', '.join(provider_instances)}.",
        target_instance_id=target_instance_id,
        slot=slot,
        provider_instance_ids=provider_instances,
    )


def _transparent_nodes(
    prompt: Mapping[str, Mapping[str, object]],
    lowered: LoweringResult,
) -> set[str]:
    """Identify wildcard outputs that select inheritable resource inputs."""

    result: set[str] = set()
    for node_id, node in prompt.items():
        definitions = lowered.node_definitions.get(node_id, {})
        if any(slot in {None, "*"} for slot in _output_slots(definitions)):
            inputs = node.get("inputs")
            if isinstance(inputs, Mapping) and any(
                _connection(value) is not None
                and _input_slot(node_id, str(name), lowered) is not None
                for name, value in inputs.items()
            ):
                result.add(node_id)
    return result


def _reverse_edges(topology: CubeTopology) -> dict[str, tuple[str, ...]]:
    """Index direct predecessors without introducing loose-node reachability."""

    result: dict[str, set[str]] = {
        instance_id: set() for instance_id in topology.instances
    }
    for edge in topology.edges:
        if (
            topology.instances[edge.source_instance_id].execution_mode == 2
            or topology.instances[edge.target_instance_id].execution_mode == 2
        ):
            continue
        result[edge.target_instance_id].add(edge.source_instance_id)
    return {
        instance_id: tuple(sorted(values)) for instance_id, values in result.items()
    }


def _inherited_key(value: InheritedBinding) -> tuple[int, str, str, str]:
    """Order reports by semantic resource before mutable prompt details."""

    return (
        _SLOT_ORDER[value.slot],
        value.target_instance_id,
        value.target_node_id,
        value.input_name,
    )
