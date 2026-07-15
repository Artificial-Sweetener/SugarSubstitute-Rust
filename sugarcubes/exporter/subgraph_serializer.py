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
"""Serialize executable Comfy subgraph definitions reachable from a cube."""

from __future__ import annotations

from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple

from .definition_snapshot import is_subgraph_wrapper_type
from .graph import CubeData, Graph


def build_subgraph_index(
    workflow: Optional[Mapping[str, Any]],
) -> Dict[str, Mapping[str, Any]]:
    """Index workflow subgraph definitions by wrapper id."""

    if not workflow or not isinstance(workflow, Mapping):
        return {}
    definitions = workflow.get("definitions")
    if not isinstance(definitions, Mapping):
        return {}
    subgraphs = definitions.get("subgraphs")
    if not isinstance(subgraphs, Sequence):
        return {}
    index: Dict[str, Mapping[str, Any]] = {}
    for entry in subgraphs:
        if not isinstance(entry, Mapping):
            continue
        sub_id = entry.get("id")
        if isinstance(sub_id, str) and sub_id:
            index[sub_id] = dict(entry)
    return index


def collect_subgraphs(
    cube: CubeData,
    graph: Graph,
    subgraph_defs: Mapping[str, Mapping[str, Any]],
) -> Tuple[List[Dict[str, Any]], List[str]]:
    """Collect executable subgraph definitions reachable from wrapper nodes."""

    warnings: List[str] = []
    root_ids: List[str] = []
    missing_wrapper_ids: set[str] = set()
    for node_id in cube.subgraph_nodes:
        node = graph.nodes.get(node_id)
        if not node:
            continue
        class_type = node.class_type
        if class_type in subgraph_defs:
            root_ids.append(class_type)
        elif is_subgraph_wrapper_type(class_type):
            missing_wrapper_ids.add(class_type)

    if missing_wrapper_ids:
        missing_text = ", ".join(sorted(missing_wrapper_ids))
        raise ValueError(
            "Workflow definitions.subgraphs is missing definition(s) for UUID "
            f"wrapper class_type(s): {missing_text}"
        )
    if not root_ids:
        return [], warnings

    ordered_ids = _collect_subgraph_dependency_order(root_ids, subgraph_defs)
    subgraphs: List[Dict[str, Any]] = []
    for sub_id in ordered_ids:
        definition = _scrub_subgraph_definition(subgraph_defs[sub_id])
        subgraphs.append(definition)
    return subgraphs, warnings


def _collect_subgraph_dependency_order(
    root_ids: Sequence[str],
    subgraph_defs: Mapping[str, Mapping[str, Any]],
) -> List[str]:
    """Return reachable subgraph ids with dependencies before dependents."""

    ordered: List[str] = []
    visiting: set[str] = set()
    visited: set[str] = set()
    missing: set[str] = set()
    stack: List[str] = []

    def visit(subgraph_id: str) -> None:
        normalized_id = subgraph_id.strip()
        if normalized_id in visited:
            return
        if normalized_id in visiting:
            cycle_start = stack.index(normalized_id) if normalized_id in stack else 0
            cycle = stack[cycle_start:] + [normalized_id]
            raise ValueError(
                "Workflow definitions.subgraphs contains cyclic nested subgraph "
                f"references: {', '.join(cycle)}"
            )

        definition = subgraph_defs.get(normalized_id)
        if not isinstance(definition, Mapping):
            missing.add(normalized_id)
            return
        if not _subgraph_has_executable_body(definition):
            raise ValueError(
                "Workflow definitions.subgraphs must include executable nodes for "
                f"wrapper class_type '{normalized_id}'"
            )

        visiting.add(normalized_id)
        stack.append(normalized_id)
        for child_id in sorted(_subgraph_wrapper_ids_in_definition(definition)):
            visit(child_id)
        stack.pop()
        visiting.remove(normalized_id)
        visited.add(normalized_id)
        ordered.append(normalized_id)

    for root_id in sorted({root_id.strip() for root_id in root_ids if root_id.strip()}):
        visit(root_id)

    if missing:
        missing_text = ", ".join(sorted(missing))
        raise ValueError(
            "Workflow definitions.subgraphs is missing definition(s) for UUID "
            f"wrapper class_type(s): {missing_text}"
        )
    return ordered


def _subgraph_wrapper_ids_in_definition(definition: Mapping[str, Any]) -> List[str]:
    """Return UUID wrapper ids referenced by one serialized subgraph body."""

    nodes_payload = definition.get("nodes")
    if not isinstance(nodes_payload, Sequence) or isinstance(
        nodes_payload, (str, bytes)
    ):
        return []

    wrapper_ids: List[str] = []
    seen_ids: set[str] = set()
    for node in nodes_payload:
        if not isinstance(node, Mapping):
            continue
        class_type = node.get("type")
        if not isinstance(class_type, str):
            class_type = node.get("class_type")
        normalized = class_type.strip() if isinstance(class_type, str) else ""
        if (
            not normalized
            or not is_subgraph_wrapper_type(normalized)
            or normalized in seen_ids
        ):
            continue
        seen_ids.add(normalized)
        wrapper_ids.append(normalized)
    return wrapper_ids


def _scrub_subgraph_definition(definition: Mapping[str, Any]) -> Dict[str, Any]:
    """Drop transient editor-only keys from persisted subgraph definitions."""

    cleaned = dict(definition)
    cleaned.pop("groups", None)
    subgraph_id = _read_string(cleaned.get("id")) or "<unknown>"
    cleaned["inputs"] = _normalize_subgraph_interface_entries(
        cleaned.get("inputs"), subgraph_id=subgraph_id, direction="inputs"
    )
    cleaned["outputs"] = _normalize_subgraph_interface_entries(
        cleaned.get("outputs"), subgraph_id=subgraph_id, direction="outputs"
    )
    raw_nodes = cleaned.get("nodes")
    if isinstance(raw_nodes, Sequence):
        scrubbed_nodes: List[Dict[str, Any]] = []
        for node in raw_nodes:
            if not isinstance(node, Mapping):
                continue
            node_copy = dict(node)
            node_copy.pop("mode", None)
            scrubbed_nodes.append(node_copy)
        cleaned["nodes"] = scrubbed_nodes
    return cleaned


def _normalize_subgraph_interface_entries(
    entries: Any, *, subgraph_id: str, direction: str
) -> List[Dict[str, Any]]:
    """Build current-format public subgraph IO entries with explicit labels."""

    if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
        return []
    normalized: List[Dict[str, Any]] = []
    seen_labels: Dict[str, str] = {}
    for index, entry in enumerate(entries):
        if not isinstance(entry, Mapping):
            continue
        name = _read_string(entry.get("name"))
        if not name:
            raise ValueError(
                f"Subgraph '{subgraph_id}' {direction} entry #{index + 1} is missing name"
            )
        label = (
            _read_string(entry.get("label"))
            or _read_string(entry.get("localized_name"))
            or name
        )
        if not label:
            raise ValueError(
                f"Subgraph '{subgraph_id}' {direction} entry '{name}' is missing label"
            )
        previous_name = seen_labels.get(label)
        if previous_name is not None:
            raise ValueError(
                f"Subgraph '{subgraph_id}' {direction} label '{label}' is used by "
                f"both '{previous_name}' and '{name}'"
            )
        seen_labels[label] = name
        normalized_entry = dict(entry)
        normalized_entry["name"] = name
        normalized_entry["label"] = label
        normalized.append(normalized_entry)
    return normalized


def _read_string(value: Any) -> str:
    """Read one trimmed string value from dynamic workflow payloads."""

    return value.strip() if isinstance(value, str) else ""


def _subgraph_has_executable_body(definition: Mapping[str, Any]) -> bool:
    """Return whether the subgraph definition contains executable node entries."""

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

