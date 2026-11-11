#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Apply validated whole-node value links without replacing graph topology."""

from __future__ import annotations

from collections.abc import Mapping, MutableMapping
from copy import deepcopy


def apply_whole_node_link(
    *,
    source_nodes: Mapping[str, object],
    source_node_key: str,
    source_label: str,
    target_nodes: Mapping[str, object],
    target_node_key: str,
    target_label: str,
) -> None:
    """Copy compatible editable state while preserving target-owned graph edges."""

    source = _node(source_nodes, source_node_key, source_label)
    target = _node(target_nodes, target_node_key, target_label)
    source_type = source.get("class_type")
    target_type = target.get("class_type")
    if source_type != target_type:
        raise ValueError(
            "Node link class types differ: "
            f"'{source_label}' is '{source_type}', '{target_label}' is '{target_type}'."
        )

    source_inputs = _inputs(source, source_label)
    target_inputs = _inputs(target, target_label)
    source_values = _editable_keys(source_inputs, source_nodes)
    target_values = _editable_keys(target_inputs, target_nodes)
    if source_values != target_values:
        raise ValueError(
            "Node link editable input keys differ for "
            f"'{source_label}' and '{target_label}'."
        )
    if _graph_signature(source_inputs, source_nodes) != _graph_signature(
        target_inputs, target_nodes
    ):
        raise ValueError(
            f"Node link graph inputs differ for '{source_label}' and '{target_label}'."
        )

    for input_key in source_values:
        target_inputs[input_key] = deepcopy(source_inputs[input_key])
    for activation_key in ("mode", "enabled"):
        if activation_key in source:
            target[activation_key] = deepcopy(source[activation_key])
        else:
            target.pop(activation_key, None)


def _node(
    nodes: Mapping[str, object], node_key: str, label: str
) -> MutableMapping[str, object]:
    """Return one mutable implementation node or reject malformed Cube state."""

    node = nodes.get(node_key)
    if not isinstance(node, MutableMapping):
        raise ValueError(f"Node link endpoint '{label}' is invalid.")
    return node


def _inputs(
    node: MutableMapping[str, object], label: str
) -> MutableMapping[str, object]:
    """Return one mutable input map without inventing missing node contracts."""

    inputs = node.get("inputs")
    if not isinstance(inputs, MutableMapping):
        raise ValueError(f"Node link endpoint '{label}' has invalid inputs.")
    return inputs


def _editable_keys(
    inputs: Mapping[str, object], nodes: Mapping[str, object]
) -> frozenset[str]:
    """Identify literal inputs whose values participate in a whole-node link."""

    return frozenset(
        input_key
        for input_key, value in inputs.items()
        if not _is_graph_link(value, nodes)
    )


def _graph_signature(
    inputs: Mapping[str, object], nodes: Mapping[str, object]
) -> tuple[tuple[str, str, int], ...]:
    """Describe graph-owned inputs using stable local symbols and output slots."""

    signature: list[tuple[str, str, int]] = []
    for input_key, value in inputs.items():
        if not _is_graph_link(value, nodes):
            continue
        assert isinstance(value, list)
        source_key, output_slot = value
        assert isinstance(source_key, str)
        assert type(output_slot) is int
        signature.append((input_key, source_key, output_slot))
    return tuple(sorted(signature))


def _is_graph_link(value: object, nodes: Mapping[str, object]) -> bool:
    """Recognize only links whose source belongs to the same Cube document."""

    return (
        isinstance(value, list)
        and len(value) == 2
        and isinstance(value[0], str)
        and type(value[1]) is int
        and value[0] in nodes
    )
