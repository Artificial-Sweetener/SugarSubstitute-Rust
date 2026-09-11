#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Validate native Comfy subgraph structure for recursive lowering."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy

from .errors import CubeLoweringError
from .lowering_values import connection, definition_input_types, node_mode, string_list


def subgraph_index(
    values: Sequence[Mapping[str, object]],
) -> dict[str, Mapping[str, object]]:
    """Index native subgraphs by stable id and reject duplicates."""

    result: dict[str, Mapping[str, object]] = {}
    for value in values:
        subgraph_id = value.get("id")
        if not isinstance(subgraph_id, str) or not subgraph_id:
            raise CubeLoweringError(
                "execution.lowering.invalid_subgraph_id",
                "Native subgraph definitions require stable string ids.",
            )
        if subgraph_id in result:
            raise CubeLoweringError(
                "execution.lowering.duplicate_subgraph_id",
                f"Native subgraph id '{subgraph_id}' is duplicated.",
            )
        result[subgraph_id] = value
    return result


def native_nodes(payload: Mapping[str, object]) -> dict[str, Mapping[str, object]]:
    """Index concrete native nodes by persisted identifier."""

    result: dict[str, Mapping[str, object]] = {}
    for value in sequence(payload.get("nodes"), "subgraph nodes"):
        if isinstance(value, Mapping):
            node_id = identifier(value.get("id"))
            if node_id is not None:
                result[node_id] = value
    return result


def native_links(payload: Mapping[str, object]) -> dict[object, Mapping[str, object]]:
    """Index current native link objects by persisted identifier."""

    result: dict[object, Mapping[str, object]] = {}
    for value in sequence(payload.get("links"), "subgraph links"):
        if isinstance(value, Mapping) and "id" in value:
            result[value["id"]] = value
    return result


def native_boundary_name(
    payload: Mapping[str, object], link: Mapping[str, object]
) -> str | None:
    """Return the public input name supplying one native boundary link."""

    if identifier(link.get("origin_id")) != "-10":
        return None
    origin_slot = link.get("origin_slot")
    inputs = sequence(payload.get("inputs"), "subgraph inputs")
    if (
        not isinstance(origin_slot, int)
        or isinstance(origin_slot, bool)
        or origin_slot < 0
        or origin_slot >= len(inputs)
    ):
        return None
    entry = inputs[origin_slot]
    if not isinstance(entry, Mapping):
        return None
    name = entry.get("name")
    return name if isinstance(name, str) and name else None


def root_bypass_value(
    node: Mapping[str, object],
    output_index: int,
    definitions: Mapping[str, object],
) -> object | None:
    """Select one compatible connected root input for a bypassed output."""

    class_type = node.get("class_type")
    definition = definitions.get(class_type) if isinstance(class_type, str) else None
    inputs = node.get("inputs")
    if not isinstance(definition, Mapping) or not isinstance(inputs, Mapping):
        return None
    outputs = string_list(definition.get("output"))
    if output_index >= len(outputs):
        return None
    types = definition_input_types(definition)
    candidates = [
        value
        for name, value in inputs.items()
        if types.get(str(name)) == outputs[output_index]
        and connection(value) is not None
    ]
    return candidates[0] if len(candidates) == 1 else None


def native_bypass_entry(
    node: Mapping[str, object],
    output_index: int,
    definitions: Mapping[str, object],
) -> Mapping[str, object] | None:
    """Select one compatible linked native input entry for bypass."""

    class_type = node.get("type")
    definition = definitions.get(class_type) if isinstance(class_type, str) else None
    definition_outputs = (
        string_list(definition.get("output")) if isinstance(definition, Mapping) else ()
    )
    output_type = (
        definition_outputs[output_index]
        if output_index < len(definition_outputs)
        else _serialized_slot_type(node.get("outputs"), output_index)
    )
    if output_type is None:
        return None
    definition_types = (
        definition_input_types(definition) if isinstance(definition, Mapping) else {}
    )
    candidates = [
        entry
        for entry in sequence(node.get("inputs"), "subgraph node inputs")
        if isinstance(entry, Mapping)
        and isinstance(entry.get("name"), str)
        and (
            definition_types.get(entry["name"])
            or (entry.get("type") if isinstance(entry.get("type"), str) else None)
        )
        == output_type
        and entry.get("link") is not None
    ]
    return candidates[0] if len(candidates) == 1 else None


def _serialized_slot_type(value: object, slot: int) -> str | None:
    """Read one native slot type when a live node definition was not embedded."""

    if (
        not isinstance(value, Sequence)
        or isinstance(value, (str, bytes, bytearray))
        or slot < 0
        or slot >= len(value)
    ):
        return None
    entry = value[slot]
    if not isinstance(entry, Mapping):
        return None
    value_type = entry.get("type")
    return value_type if isinstance(value_type, str) and value_type else None


def class_type(node: Mapping[str, object], path: str) -> str:
    """Return one root or native node class type with a located failure."""

    value = node.get("class_type", node.get("type"))
    if not isinstance(value, str) or not value.strip():
        raise CubeLoweringError(
            "execution.lowering.invalid_class_type",
            f"Node at '{path}' has no valid class type.",
            path=path,
        )
    return value.strip()


def string_mapping(value: object, path: str) -> Mapping[str, object]:
    """Return one string-keyed implementation object."""

    if not isinstance(value, Mapping) or any(not isinstance(key, str) for key in value):
        raise CubeLoweringError(
            "execution.lowering.invalid_inputs",
            f"Node inputs at '{path}' must be an object.",
            path=path,
        )
    return value


def definition(
    definitions: Mapping[str, object], value_type: str
) -> Mapping[str, object]:
    """Copy one embedded live node definition when available."""

    value = definitions.get(value_type)
    return deepcopy(dict(value)) if isinstance(value, Mapping) else {}


def native_name(node: Mapping[str, object], fallback: str) -> str:
    """Prefer a stable SugarCubes symbol over a native numeric id."""

    properties = node.get("properties")
    if isinstance(properties, Mapping):
        symbol = properties.get("sugarcubes_symbol")
        if isinstance(symbol, str) and symbol.strip():
            return symbol.strip()
    return fallback


def sequence(value: object, label: str) -> Sequence[object]:
    """Return one untrusted native array or fail with stable context."""

    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise CubeLoweringError(
            "execution.lowering.invalid_subgraph",
            f"Native {label} must be an array.",
        )
    return value


def identifier(value: object) -> str | None:
    """Normalize one native integer or string node identifier."""

    if isinstance(value, bool) or not isinstance(value, (str, int)):
        return None
    normalized = str(value).strip()
    return normalized or None


def mode(node: Mapping[str, object]) -> int:
    """Return one authored native execution mode."""

    return node_mode(node)


def node_id(instance_id: str, symbol: str) -> str:
    """Build one deterministic outer Cube execution node id."""

    return f"{instance_id}:{symbol}"


def cycle(identity: str) -> CubeLoweringError:
    """Create one stable recursive subgraph diagnostic."""

    return CubeLoweringError(
        "execution.lowering.subgraph_cycle",
        f"Native subgraph output resolution cycles at '{identity}'.",
    )


def missing_output(identity: str, output_index: int) -> CubeLoweringError:
    """Create one stable disconnected or absent output diagnostic."""

    return CubeLoweringError(
        "execution.lowering.missing_subgraph_output",
        f"Native subgraph '{identity}' has no connected output {output_index}.",
    )
