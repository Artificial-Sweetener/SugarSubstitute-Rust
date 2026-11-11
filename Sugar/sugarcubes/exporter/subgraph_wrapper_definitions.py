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
"""Derive exact node definitions for serialized native subgraph wrappers."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from copy import deepcopy
from typing import Any, TypeGuard

from ..cube_model.picker_fields import find_input_field_spec
from ..cube_model.subgraph_boundary_widgets import index_boundary_widget_targets
from .definition_snapshot import (
    BindingResolver,
    collect_definitions,
    collect_subgraph_node_types,
    normalize_definition_snapshot,
)
from .graph import Graph


def collect_cube_definitions(
    symbols: Mapping[str, str],
    graph: Graph,
    resolver: BindingResolver | None,
    subgraphs: Sequence[Mapping[str, Any]],
) -> tuple[dict[str, Any], dict[str, Any], list[str]]:
    """Collect exact direct, nested, and wrapper definitions for one Cube."""

    definitions, validation_definitions, warnings = collect_definitions(
        symbols,
        graph,
        resolver,
        extra_class_types=collect_subgraph_node_types(subgraphs),
    )
    wrappers = build_subgraph_wrapper_definitions(subgraphs, validation_definitions)
    validation_definitions.update(wrappers)
    definitions.update(
        {
            class_type: normalize_definition_snapshot(class_type, definition)
            for class_type, definition in wrappers.items()
        }
    )
    return definitions, validation_definitions, warnings


def build_subgraph_wrapper_definitions(
    subgraphs: Sequence[Mapping[str, Any]],
    definitions: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    """Build wrapper definitions from public boundaries and exact inner fields.

    Subgraphs must be ordered with dependencies before their consumers. This is
    the order produced by the canonical subgraph collector and lets nested
    wrappers inherit exact field semantics without consulting host-global state.
    """

    available = dict(definitions)
    wrappers: dict[str, dict[str, Any]] = {}
    for subgraph in subgraphs:
        wrapper_id = _text(subgraph.get("id"))
        if not wrapper_id:
            raise ValueError("Serialized subgraph definition is missing its wrapper id")
        wrapper = _build_wrapper_definition(subgraph, available)
        wrappers[wrapper_id] = wrapper
        available[wrapper_id] = wrapper
    return wrappers


def _build_wrapper_definition(
    subgraph: Mapping[str, Any],
    definitions: Mapping[str, Any],
) -> dict[str, Any]:
    """Build one wrapper definition using its stable boundary identities."""

    wrapper_id = _text(subgraph.get("id"))
    boundary_targets = index_boundary_widget_targets(subgraph)
    nodes = _nodes_by_id(subgraph.get("nodes"))
    required: dict[str, Any] = {}
    input_order: list[str] = []
    for boundary in _records(subgraph.get("inputs")):
        name = _text(boundary.get("name"))
        if not name:
            raise ValueError(f"Subgraph wrapper '{wrapper_id}' has an unnamed input")
        if name in required:
            raise ValueError(
                f"Subgraph wrapper '{wrapper_id}' has duplicate input '{name}'"
            )
        target = boundary_targets.get(name)
        required[name] = (
            _target_field_spec(wrapper_id, name, target, nodes, definitions)
            if target is not None
            else _boundary_field_spec(boundary)
        )
        input_order.append(name)

    outputs = _records(subgraph.get("outputs"))
    output_types = [_text(output.get("type")) or "*" for output in outputs]
    output_names = [
        _text(output.get("name")) or _text(output.get("label")) or output_type
        for output, output_type in zip(outputs, output_types)
    ]
    name = _text(subgraph.get("name")) or wrapper_id
    return {
        "input": {"required": required},
        "input_order": {"required": input_order},
        "output": output_types,
        "output_name": output_names,
        "output_is_list": [False] * len(output_types),
        "name": wrapper_id,
        "display_name": name,
        "python_module": "comfy.subgraph",
        "category": "subgraph",
        "output_node": False,
    }


def _target_field_spec(
    wrapper_id: str,
    boundary_name: str,
    target: tuple[str | int, str],
    nodes: Mapping[str | int, Mapping[str, Any]],
    definitions: Mapping[str, Any],
) -> Any:
    """Resolve one exposed widget to the exact inner node field specification."""

    node_id, input_name = target
    node = nodes.get(node_id)
    if node is None:
        raise ValueError(
            f"Subgraph wrapper '{wrapper_id}' input '{boundary_name}' targets "
            f"missing node '{node_id}'"
        )
    class_type = node.get("type", node.get("class_type"))
    definition = definitions.get(class_type) if isinstance(class_type, str) else None
    if not isinstance(class_type, str) or not isinstance(definition, Mapping):
        raise ValueError(
            f"Subgraph wrapper '{wrapper_id}' input '{boundary_name}' lacks "
            f"the exact target definition '{class_type}'"
        )
    field_spec = find_input_field_spec(definition, input_name)
    if field_spec is None:
        raise ValueError(
            f"Subgraph wrapper '{wrapper_id}' input '{boundary_name}' targets "
            f"unknown field '{class_type}.{input_name}'"
        )
    return deepcopy(field_spec)


def _boundary_field_spec(boundary: Mapping[str, Any]) -> list[Any]:
    """Represent a socket-only boundary without inventing widget semantics."""

    input_type = _text(boundary.get("type")) or "*"
    return [input_type, {"forceInput": True}]


def _nodes_by_id(value: object) -> dict[str | int, Mapping[str, Any]]:
    """Index serialized native nodes without coercing their identities."""

    return {
        node_id: node
        for node in _records(value)
        if isinstance(node.get("id"), str | int)
        and not isinstance(node.get("id"), bool)
        if (node_id := node.get("id")) is not None
    }


def _records(value: object) -> list[Mapping[str, Any]]:
    """Return mapping entries from one non-string sequence."""

    if not _is_sequence(value):
        return []
    return [entry for entry in value if isinstance(entry, Mapping)]


def _text(value: object) -> str:
    """Return one normalized text value."""

    return value.strip() if isinstance(value, str) else ""


def _is_sequence(value: object) -> TypeGuard[Sequence[Any]]:
    """Narrow non-string sequences."""

    return isinstance(value, Sequence) and not isinstance(value, str | bytes)
