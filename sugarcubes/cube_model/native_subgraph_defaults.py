#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Project Cube-authored native wrapper defaults by stable boundary identity."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence, Set
from copy import deepcopy
from typing import TypeGuard

from .picker_fields import find_input_field_spec, resolve_picker_fallback
from .subgraph_boundary_widgets import (
    index_boundary_widget_names,
    index_boundary_widget_targets,
)
from .widget_values import WidgetSnapshotError, decode_versioned_widget_snapshot


class NativeSubgraphDefaultError(ValueError):
    """Reject an embedded default that cannot be identified without guessing."""


def native_widget_defaults(
    node: Mapping[str, object],
    definition: Mapping[str, object],
    boundary_names: Set[str],
    *,
    versioned_widget_names: Sequence[str] | None = None,
) -> dict[str, object]:
    """Decode one node's exact saved defaults without positional guessing."""

    try:
        snapshot = decode_versioned_widget_snapshot(
            node,
            definition,
            included_linked_names=boundary_names,
            versioned_widget_names=versioned_widget_names,
        )
    except WidgetSnapshotError as error:
        raise NativeSubgraphDefaultError(
            f"Native subgraph widget defaults are ambiguous: {error}"
        ) from error
    if snapshot is None:
        return {}
    return {
        name: _definition_default(definition, name, value)
        for name, value in snapshot.values.items()
    }


def native_boundary_widget_names(
    node: Mapping[str, object],
    links: Mapping[object, Mapping[str, object]],
) -> frozenset[str]:
    """Return widget names whose links originate at the wrapper boundary."""

    entries = node.get("inputs")
    if not _is_sequence(entries):
        return frozenset()
    indexed_links = {
        identity: link
        for key, link in links.items()
        if (identity := _identifier(key)) is not None
    }
    return frozenset(
        name
        for entry in entries
        if isinstance(entry, Mapping)
        if isinstance(entry.get("widget"), Mapping)
        if (name := _name(entry.get("name"))) is not None
        if (link_id := _identifier(entry.get("link"))) is not None
        if (link := indexed_links.get(link_id)) is not None
        if _identifier(link.get("origin_id")) == "-10"
    )


def native_subgraph_defaults(
    subgraph: Mapping[str, object],
    definitions: Mapping[str, object],
    *,
    resolve_live_definition: Callable[[str], Mapping[str, object] | None] | None = None,
) -> dict[str, object]:
    """Return every public wrapper default stored in one exact Cube subgraph."""

    nodes = _nodes_by_id(subgraph.get("nodes"))
    targets = index_boundary_widget_targets(subgraph)
    boundary_names = index_boundary_widget_names(subgraph)
    result: dict[str, object] = {}
    for boundary_name, (node_id, input_name) in targets.items():
        node = nodes.get(node_id)
        if node is None:
            raise NativeSubgraphDefaultError(
                f"Boundary '{boundary_name}' targets missing node '{node_id}'"
            )
        class_type = node.get("type", node.get("class_type"))
        definition = (
            definitions.get(class_type) if isinstance(class_type, str) else None
        )
        if not isinstance(class_type, str) or not isinstance(definition, Mapping):
            raise NativeSubgraphDefaultError(
                f"Boundary '{boundary_name}' lacks exact node definition '{class_type}'"
            )
        live_definition = (
            resolve_live_definition(class_type)
            if resolve_live_definition is not None
            else None
        )
        default_definition = (
            live_definition if isinstance(live_definition, Mapping) else definition
        )
        try:
            snapshot = decode_versioned_widget_snapshot(
                node,
                definition,
                included_linked_names=boundary_names.get(node_id, frozenset()),
            )
        except WidgetSnapshotError as error:
            raise NativeSubgraphDefaultError(
                f"Boundary '{boundary_name}' has ambiguous saved value: {error}"
            ) from error
        if snapshot is None or input_name not in snapshot.values:
            raise NativeSubgraphDefaultError(
                f"Boundary '{boundary_name}' lacks saved widget '{input_name}'"
            )
        result[boundary_name] = _definition_default(
            default_definition,
            input_name,
            snapshot.values[input_name],
        )
    return result


def _nodes_by_id(value: object) -> dict[str | int, Mapping[str, object]]:
    """Index valid serialized nodes without coercing graph identities."""

    if not isinstance(value, list):
        return {}
    return {
        node_id: node
        for node in value
        if isinstance(node, Mapping)
        if isinstance(node.get("id"), str | int)
        and not isinstance(node.get("id"), bool)
        if (node_id := node.get("id")) is not None
    }


def _definition_default(
    definition: Mapping[str, object],
    input_name: str,
    persisted_value: object,
) -> object:
    """Use a declared default only for an intentionally empty snapshot slot."""

    if persisted_value is not None:
        return deepcopy(persisted_value)
    field_spec = find_input_field_spec(definition, input_name)
    if _is_sequence(field_spec) and len(field_spec) > 1:
        metadata = field_spec[1]
        if isinstance(metadata, Mapping) and "default" in metadata:
            return deepcopy(metadata["default"])
    picker = resolve_picker_fallback(field_spec)
    if picker is not None:
        return deepcopy(picker.value)
    return None


def _identifier(value: object) -> str | None:
    """Normalize native graph identifiers without accepting booleans."""

    if isinstance(value, bool):
        return None
    return str(value) if isinstance(value, str | int) else None


def _name(value: object) -> str | None:
    """Return one nonempty native input name."""

    return value if isinstance(value, str) and value else None


def _is_sequence(value: object) -> TypeGuard[Sequence[object]]:
    """Narrow non-string serialized sequences."""

    return isinstance(value, Sequence) and not isinstance(
        value, (str, bytes, bytearray)
    )
