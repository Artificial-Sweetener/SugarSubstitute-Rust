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
"""Analyze selected Cube graph and native-subgraph runtime contracts."""

from __future__ import annotations

import re
from collections.abc import Collection, Mapping, Sequence
from typing import Any

from ...exporter.graph import CubeAnalysis
from .cube_metadata import normalize_metadata_string

_UUID_CLASS_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)


def extract_uuid_wrapper_classes(graph: Mapping[str, Any]) -> set[str]:
    """Return UUID wrapper class types referenced by the graph."""

    wrappers: set[str] = set()
    for node_id, node in graph.items():
        if node_id == "workflow" or not isinstance(node, Mapping):
            continue
        class_type = node.get("class_type")
        class_name = class_type.strip() if isinstance(class_type, str) else ""
        if class_name and _UUID_CLASS_RE.match(class_name):
            wrappers.add(class_name)
    return wrappers


def index_workflow_subgraphs(
    workflow: Mapping[str, Any],
) -> dict[str, Mapping[str, Any]]:
    """Index workflow subgraph definitions by id."""

    definitions = workflow.get("definitions")
    if not isinstance(definitions, Mapping):
        return {}
    subgraphs = definitions.get("subgraphs")
    if not isinstance(subgraphs, Sequence):
        return {}
    index: dict[str, Mapping[str, Any]] = {}
    for entry in subgraphs:
        if not isinstance(entry, Mapping):
            continue
        subgraph_id = entry.get("id")
        if isinstance(subgraph_id, str) and subgraph_id:
            index[subgraph_id] = dict(entry)
    return index


def subgraph_has_executable_body(definition: Mapping[str, Any]) -> bool:
    """Return whether a subgraph definition contains executable nodes."""

    nodes = definition.get("nodes")
    if not isinstance(nodes, Sequence):
        return False
    for node in nodes:
        if not isinstance(node, Mapping):
            continue
        node_type = node.get("type")
        if not isinstance(node_type, str):
            node_type = node.get("class_type")
        if isinstance(node_type, str) and node_type.strip():
            return True
    return False


def collect_subgraph_contract_violations(
    graph: Mapping[str, Any],
    workflow: Mapping[str, Any],
) -> dict[str, list[str]]:
    """Collect malformed subgraph wrapper definitions."""

    wrapper_ids = extract_uuid_wrapper_classes(graph)
    if not wrapper_ids:
        return {}
    indexed = index_workflow_subgraphs(workflow)
    missing = sorted(wrapper_ids - set(indexed.keys()))
    empty = sorted(
        wrapper_id
        for wrapper_id in wrapper_ids & set(indexed.keys())
        if not subgraph_has_executable_body(indexed[wrapper_id])
    )
    violations: dict[str, list[str]] = {}
    if missing:
        violations["missing_subgraphs"] = missing
    if empty:
        violations["empty_subgraph_bodies"] = empty
    label_violations = collect_subgraph_interface_label_violations(
        indexed, sorted(wrapper_ids & set(indexed.keys()))
    )
    violations.update(label_violations)
    return violations


def collect_subgraph_interface_label_violations(
    indexed: Mapping[str, Mapping[str, Any]], wrapper_ids: Sequence[str]
) -> dict[str, list[str]]:
    """Collect missing or duplicate public subgraph IO labels."""

    missing_labels: list[str] = []
    duplicate_labels: list[str] = []
    for wrapper_id in sorted(set(wrapper_ids)):
        definition = indexed.get(wrapper_id)
        if not isinstance(definition, Mapping):
            continue
        for direction in ("inputs", "outputs"):
            entries = definition.get(direction)
            if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
                continue
            labels: dict[str, str] = {}
            for index, entry in enumerate(entries):
                if not isinstance(entry, Mapping):
                    continue
                name = normalize_metadata_string(entry.get("name"))
                label = normalize_metadata_string(entry.get("label"))
                slot = name or f"#{index + 1}"
                if not name or not label:
                    missing_labels.append(f"{wrapper_id}.{direction}.{slot}")
                    continue
                previous = labels.get(label)
                if previous is not None:
                    duplicate_labels.append(
                        f"{wrapper_id}.{direction}.{label}: {previous}, {name}"
                    )
                labels[label] = name
    violations: dict[str, list[str]] = {}
    if missing_labels:
        violations["missing_subgraph_labels"] = missing_labels
    if duplicate_labels:
        violations["duplicate_subgraph_labels"] = duplicate_labels
    return violations


def collect_subgraph_node_class_types(workflow: Mapping[str, Any]) -> set[str]:
    """Collect concrete node class types declared in workflow subgraphs."""

    required: set[str] = set()
    for definition in index_workflow_subgraphs(workflow).values():
        nodes = definition.get("nodes")
        if not isinstance(nodes, Sequence):
            continue
        for node in nodes:
            if not isinstance(node, Mapping):
                continue
            class_type = node.get("type")
            normalized = class_type.strip() if isinstance(class_type, str) else ""
            if normalized and not _UUID_CLASS_RE.match(normalized):
                required.add(normalized)
    return required


def collect_required_node_class_types(
    graph: Mapping[str, Any],
    workflow: Mapping[str, Any],
) -> set[str]:
    """Collect all runtime class types needed for export validation."""

    required = collect_subgraph_node_class_types(workflow)
    for node_id, node in graph.items():
        if node_id == "workflow" or not isinstance(node, Mapping):
            continue
        class_type = node.get("class_type")
        normalized = class_type.strip() if isinstance(class_type, str) else ""
        if normalized and not _UUID_CLASS_RE.match(normalized):
            required.add(normalized)
    return required


def collect_selected_cube_ids(
    analysis: CubeAnalysis, cube_ids: Collection[str]
) -> list[str]:
    """Return requested cube ids that exist in the analyzed graph."""

    requested_ids = {cube_id for cube_id in cube_ids if cube_id}
    return sorted(cube_id for cube_id in requested_ids if cube_id in analysis.cubes)


def collect_selected_cube_wrapper_classes(
    analysis: CubeAnalysis, cube_ids: Collection[str]
) -> set[str]:
    """Collect UUID wrapper class types used by the selected cube set."""

    wrapper_ids: set[str] = set()
    for cube_id in collect_selected_cube_ids(analysis, cube_ids):
        cube = analysis.cubes[cube_id]
        for node_id in cube.subgraph_nodes:
            node = analysis.graph.nodes.get(node_id)
            if not node:
                continue
            class_name = node.class_type.strip()
            if class_name and _UUID_CLASS_RE.match(class_name):
                wrapper_ids.add(class_name)
    return wrapper_ids


def collect_selected_cube_subgraph_node_class_types(
    workflow: Mapping[str, Any], wrapper_ids: Sequence[str]
) -> set[str]:
    """Collect concrete class types from selected wrapper subgraph definitions."""

    indexed = index_workflow_subgraphs(workflow)
    required: set[str] = set()
    for wrapper_id in sorted(set(wrapper_ids)):
        definition = indexed.get(wrapper_id)
        if not isinstance(definition, Mapping):
            continue
        nodes = definition.get("nodes")
        if not isinstance(nodes, Sequence):
            continue
        for node in nodes:
            if not isinstance(node, Mapping):
                continue
            class_type = node.get("type")
            if not isinstance(class_type, str):
                class_type = node.get("class_type")
            normalized = class_type.strip() if isinstance(class_type, str) else ""
            if normalized and not _UUID_CLASS_RE.match(normalized):
                required.add(normalized)
    return required


def collect_selected_cube_required_node_class_types(
    analysis: CubeAnalysis,
    workflow: Mapping[str, Any],
    cube_ids: Collection[str],
) -> set[str]:
    """Collect runtime class types required by the selected cube set only."""

    required: set[str] = set()
    wrapper_ids = collect_selected_cube_wrapper_classes(analysis, cube_ids)
    for cube_id in collect_selected_cube_ids(analysis, cube_ids):
        cube = analysis.cubes[cube_id]
        for node_id in cube.subgraph_nodes:
            node = analysis.graph.nodes.get(node_id)
            if not node:
                continue
            normalized = node.class_type.strip()
            if normalized and not _UUID_CLASS_RE.match(normalized):
                required.add(normalized)
    required.update(
        collect_selected_cube_subgraph_node_class_types(workflow, sorted(wrapper_ids))
    )
    return required


def collect_selected_cube_subgraph_contract_violations(
    analysis: CubeAnalysis,
    workflow: Mapping[str, Any],
    cube_ids: Collection[str],
) -> dict[str, list[str]]:
    """Collect malformed wrapper definitions for the selected cube set."""

    wrapper_ids = collect_selected_cube_wrapper_classes(analysis, cube_ids)
    if not wrapper_ids:
        return {}
    indexed = index_workflow_subgraphs(workflow)
    missing = sorted(wrapper_ids - set(indexed.keys()))
    empty = sorted(
        wrapper_id
        for wrapper_id in wrapper_ids & set(indexed.keys())
        if not subgraph_has_executable_body(indexed[wrapper_id])
    )
    violations: dict[str, list[str]] = {}
    if missing:
        violations["missing_subgraphs"] = missing
    if empty:
        violations["empty_subgraph_bodies"] = empty
    label_violations = collect_subgraph_interface_label_violations(
        indexed, sorted(wrapper_ids & set(indexed.keys()))
    )
    violations.update(label_violations)
    return violations


def collect_missing_node_class_types(
    class_types: Collection[str],
    node_class_mappings: Mapping[str, Any],
) -> list[str]:
    """Return sorted class types missing from the active Comfy registry."""

    return sorted(
        class_type
        for class_type in set(class_types)
        if class_type not in node_class_mappings
    )
