#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Recover saved Cube values through exact versioned semantic identities."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from typing import cast

from ..cube_model.native_subgraph_defaults import (
    native_boundary_widget_names,
    native_widget_defaults,
)


class LegacyWorkflowImportError(ValueError):
    """Reject legacy workflow state that cannot be reconciled without guessing."""


def reconcile_saved_node_values(
    saved_node: Mapping[str, object],
    target_node: Mapping[str, object],
    definitions: Mapping[str, object],
    native_subgraphs: Mapping[str, Mapping[str, object]],
    exact_subgraphs: Mapping[str, Mapping[str, object]],
    versioned_widget_names: Sequence[str] | None,
) -> dict[str, object]:
    """Decode direct or nested saved values using an exact co-versioned schema."""

    properties = saved_node.get("properties")
    property_map = properties if isinstance(properties, Mapping) else {}
    original_subgraph_id = _text(property_map.get("sugarcubes_original_subgraph_id"))
    if original_subgraph_id:
        return _reconcile_nested_values(
            saved_node,
            original_subgraph_id,
            definitions,
            native_subgraphs,
            exact_subgraphs,
        )

    class_type = _text(target_node.get("class_type"))
    definition = definitions.get(class_type)
    if not isinstance(definition, Mapping):
        values = saved_node.get("widgets_values")
        if _sequence(values):
            raise LegacyWorkflowImportError(
                f"Node '{saved_symbol(saved_node)}' lacks its exact saved schema"
            )
        return {}
    try:
        return native_widget_defaults(
            saved_node,
            definition,
            frozenset(),
            versioned_widget_names=versioned_widget_names,
        )
    except ValueError as error:
        raise LegacyWorkflowImportError(
            f"Node '{saved_symbol(saved_node)}' values are ambiguous: {error}"
        ) from error


def surface_widget_names(payload: Mapping[str, object]) -> dict[str, list[str]]:
    """Index exact Cube surface controls as historical widget identities."""

    surface = payload.get("surface")
    surface_map = surface if isinstance(surface, Mapping) else {}
    result: dict[str, list[str]] = {}
    for control in _records(surface_map.get("controls")):
        symbol = _text(control.get("symbol"))
        input_name = _text(control.get("input_name"))
        if symbol and input_name and input_name not in result.setdefault(symbol, []):
            result[symbol].append(input_name)
    return result


def saved_symbol(node: Mapping[str, object]) -> str:
    """Read the stable Cube node symbol from a saved workflow node."""

    properties = node.get("properties")
    return (
        _text(properties.get("sugarcubes_symbol"))
        if isinstance(properties, Mapping)
        else ""
    )


def _reconcile_nested_values(
    saved_node: Mapping[str, object],
    original_subgraph_id: str,
    definitions: Mapping[str, object],
    native_subgraphs: Mapping[str, Mapping[str, object]],
    exact_subgraphs: Mapping[str, Mapping[str, object]],
) -> dict[str, object]:
    """Recover one nested wrapper from its saved clone and exact Cube definition."""

    clone_id = _text(saved_node.get("type"))
    clone = native_subgraphs.get(clone_id)
    if clone is None:
        raise LegacyWorkflowImportError(
            f"Nested node '{saved_symbol(saved_node)}' lacks definition '{clone_id}'"
        )
    exact = exact_subgraphs.get(original_subgraph_id)
    if exact is None:
        raise LegacyWorkflowImportError(
            f"Nested node '{saved_symbol(saved_node)}' exact Cube version lacks "
            f"subgraph '{original_subgraph_id}'"
        )
    exact_defaults = _native_subgraph_boundary_defaults(exact, definitions)
    saved_defaults = _native_subgraph_boundary_defaults(
        clone,
        definitions,
        allowed_names=frozenset(exact_defaults),
    )
    missing = set(exact_defaults) - set(saved_defaults)
    if missing:
        names = ", ".join(sorted(missing))
        raise LegacyWorkflowImportError(
            f"Nested node '{saved_symbol(saved_node)}' lacks saved values for: {names}"
        )
    return {
        name: deepcopy(value)
        for name, value in saved_defaults.items()
        if value != exact_defaults[name]
    }


def _native_subgraph_boundary_defaults(
    subgraph: Mapping[str, object],
    definitions: Mapping[str, object],
    *,
    allowed_names: frozenset[str] | None = None,
) -> dict[str, object]:
    """Follow native boundary links to named inner widget values."""

    nodes = _index_records(subgraph.get("nodes"), "subgraph node")
    indexed_links = _index_records(subgraph.get("links"), "subgraph link")
    links = cast(Mapping[object, Mapping[str, object]], indexed_links)
    result: dict[str, object] = {}
    for boundary in _records(subgraph.get("inputs")):
        boundary_name = _text(boundary.get("name"))
        if allowed_names is not None and boundary_name not in allowed_names:
            continue
        resolved = _resolve_boundary_values(
            boundary,
            boundary_name,
            nodes,
            links,
            definitions,
        )
        if not resolved:
            continue
        if any(value != resolved[0] for value in resolved[1:]):
            raise LegacyWorkflowImportError(
                f"Nested input '{boundary_name}' has conflicting saved defaults"
            )
        result[boundary_name] = deepcopy(resolved[0])
    return result


def _resolve_boundary_values(
    boundary: Mapping[str, object],
    boundary_name: str,
    nodes: Mapping[str, Mapping[str, object]],
    links: Mapping[object, Mapping[str, object]],
    definitions: Mapping[str, object],
) -> list[object]:
    """Resolve every saved inner target for one public boundary."""

    resolved: list[object] = []
    for link_id in _identifiers(boundary.get("linkIds")):
        link = links.get(link_id)
        if link is None or _identifier(link.get("origin_id")) != "-10":
            continue
        target = nodes.get(_identifier(link.get("target_id")))
        target_slot = _integer(link.get("target_slot"))
        inputs = _records(target.get("inputs")) if target is not None else []
        target_input = inputs[target_slot] if 0 <= target_slot < len(inputs) else None
        if target is None or target_input is None:
            continue
        widget = target_input.get("widget")
        if not isinstance(widget, Mapping):
            continue
        input_name = _text(widget.get("name")) or _text(target_input.get("name"))
        class_type = _text(target.get("type", target.get("class_type")))
        definition = definitions.get(class_type)
        if not input_name or not isinstance(definition, Mapping):
            raise LegacyWorkflowImportError(
                f"Nested input '{boundary_name}' lacks exact inner-node schema"
            )
        defaults = native_widget_defaults(
            target,
            definition,
            native_boundary_widget_names(target, links),
        )
        if input_name in defaults:
            resolved.append(defaults[input_name])
    return resolved


def _index_records(
    value: object,
    label: str,
) -> dict[str, Mapping[str, object]]:
    """Index saved records by graph ID while rejecting duplicates."""

    result: dict[str, Mapping[str, object]] = {}
    for record in _records(value):
        identity = _identifier(record.get("id"))
        if not identity:
            continue
        if identity in result:
            raise LegacyWorkflowImportError(f"Duplicate {label} id '{identity}'")
        result[identity] = record
    return result


def _records(value: object) -> list[Mapping[str, object]]:
    """Return only mapping entries from one untrusted sequence."""

    return [entry for entry in _sequence(value) if isinstance(entry, Mapping)]


def _sequence(value: object) -> Sequence[object]:
    """Return one non-string sequence or an empty tuple."""

    return (
        value
        if isinstance(value, Sequence) and not isinstance(value, str | bytes)
        else ()
    )


def _identifiers(value: object) -> list[str]:
    """Normalize graph identifiers from one untrusted sequence."""

    return [identity for entry in _sequence(value) if (identity := _identifier(entry))]


def _identifier(value: object) -> str:
    """Normalize one graph identifier without accepting booleans."""

    return (
        str(value)
        if isinstance(value, str | int) and not isinstance(value, bool)
        else ""
    )


def _integer(value: object) -> int:
    """Return one nonnegative graph slot or a negative sentinel."""

    return (
        value
        if isinstance(value, int) and not isinstance(value, bool) and value >= 0
        else -1
    )


def _text(value: object) -> str:
    """Return one trimmed string."""

    return value.strip() if isinstance(value, str) else ""
