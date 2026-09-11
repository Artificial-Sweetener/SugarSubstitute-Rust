#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Resolve stable Cube fields across direct nodes and native subgraph wrappers."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from dataclasses import dataclass
from typing import TypeGuard

from ..cube_model.native_subgraph_defaults import native_subgraph_defaults
from ..cube_model.subgraph_boundary_widgets import (
    index_boundary_widget_names,
    index_boundary_widget_targets,
)
from ..cube_model.widget_values import (
    WORKFLOW_WIDGET_VALUES_KEY,
    decode_versioned_widget_snapshot,
)


@dataclass(frozen=True, slots=True)
class StableDocumentFieldValue:
    """Describe whether one stable field resolved and its detached value."""

    found: bool
    value: object = None


def read_stable_document_field(
    document: Mapping[str, object],
    node_symbol: str,
    input_name: str,
) -> StableDocumentFieldValue:
    """Read one direct input or native wrapper boundary by stable identity."""

    node, implementation = _node_and_implementation(document, node_symbol)
    if node is None or implementation is None:
        return StableDocumentFieldValue(False)
    inputs = node.get("inputs")
    if isinstance(inputs, Mapping) and input_name in inputs:
        return StableDocumentFieldValue(True, deepcopy(inputs[input_name]))
    subgraph = _native_subgraph(implementation, node)
    definitions = implementation.get("definitions")
    if subgraph is None or not isinstance(definitions, Mapping):
        return StableDocumentFieldValue(False)
    try:
        values = native_subgraph_defaults(subgraph, definitions)
    except ValueError:
        return StableDocumentFieldValue(False)
    if input_name not in values:
        return StableDocumentFieldValue(False)
    return StableDocumentFieldValue(True, deepcopy(values[input_name]))


def write_stable_document_field(
    document: dict[str, object],
    node_symbol: str,
    input_name: str,
    value: object,
) -> bool:
    """Write one direct input or native wrapper boundary by stable identity."""

    node, implementation = _node_and_implementation(document, node_symbol)
    if node is None or implementation is None:
        return False
    inputs = node.get("inputs")
    if isinstance(inputs, dict) and input_name in inputs:
        inputs[input_name] = deepcopy(value)
        return True
    subgraph = _native_subgraph(implementation, node)
    definitions = implementation.get("definitions")
    if not isinstance(subgraph, dict) or not isinstance(definitions, Mapping):
        return False
    target = index_boundary_widget_targets(subgraph).get(input_name)
    if target is None:
        return False
    node_id, nested_input_name = target
    nested_node = _native_node(subgraph, node_id)
    if nested_node is None:
        return False
    class_type = nested_node.get("type", nested_node.get("class_type"))
    definition = definitions.get(class_type) if isinstance(class_type, str) else None
    boundary_names = index_boundary_widget_names(subgraph).get(node_id, frozenset())
    if not isinstance(definition, Mapping):
        return False
    try:
        snapshot = decode_versioned_widget_snapshot(
            nested_node,
            definition,
            included_linked_names=boundary_names,
        )
    except ValueError:
        return False
    if snapshot is None or nested_input_name not in snapshot.values:
        return False
    named_values = deepcopy(snapshot.values)
    named_values[nested_input_name] = deepcopy(value)
    nested_node[WORKFLOW_WIDGET_VALUES_KEY] = named_values
    return True


def _node_and_implementation(
    document: Mapping[str, object], node_symbol: str
) -> tuple[dict[str, object] | None, dict[str, object] | None]:
    """Return one mutable implementation node and its owning document section."""

    implementation = document.get("implementation")
    nodes = implementation.get("nodes") if isinstance(implementation, dict) else None
    node = nodes.get(node_symbol) if isinstance(nodes, Mapping) else None
    return (
        node if isinstance(node, dict) else None,
        implementation if isinstance(implementation, dict) else None,
    )


def _native_subgraph(
    implementation: Mapping[str, object], node: Mapping[str, object]
) -> Mapping[str, object] | None:
    """Resolve the native wrapper definition used by one implementation node."""

    class_type = node.get("class_type")
    subgraphs = implementation.get("subgraphs")
    if not isinstance(class_type, str) or not _is_sequence(subgraphs):
        return None
    return next(
        (
            subgraph
            for subgraph in subgraphs
            if isinstance(subgraph, Mapping) and subgraph.get("id") == class_type
        ),
        None,
    )


def _native_node(
    subgraph: Mapping[str, object], node_id: str | int
) -> dict[str, object] | None:
    """Return one mutable native node without coercing graph identities."""

    nodes = subgraph.get("nodes")
    if not _is_sequence(nodes):
        return None
    return next(
        (
            node
            for node in nodes
            if isinstance(node, dict) and node.get("id") == node_id
        ),
        None,
    )


def _is_sequence(value: object) -> TypeGuard[Sequence[object]]:
    """Return whether one value is a non-string sequence."""

    return isinstance(value, Sequence) and not isinstance(
        value, (str, bytes, bytearray)
    )


__all__ = [
    "StableDocumentFieldValue",
    "read_stable_document_field",
    "write_stable_document_field",
]
