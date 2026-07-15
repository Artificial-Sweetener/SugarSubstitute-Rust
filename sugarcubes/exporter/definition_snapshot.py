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
"""Collect portable node-definition snapshots for cube exports."""

from __future__ import annotations

import importlib
import re
from copy import deepcopy
from typing import Any, Callable, Dict, List, Mapping, Optional, Sequence, Tuple

from ..cube_model.picker_fields import compact_picker_field_spec
from .graph import Graph

BindingResolver = Callable[[str], Mapping[str, Any]]

_UUID_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)
_DEFINITION_HELP_KEYS = frozenset({"tooltip", "output_tooltips", "description"})
_COMFY_RUNTIME_RESOLVED = False
_COMFY_NODES_MODULE: Any = None
_COMFY_NODE_INTERNAL_TYPE: Any = None


def collect_definitions(
    symbols: Mapping[str, str],
    graph: Graph,
    resolver: Optional[BindingResolver],
    *,
    extra_class_types: Optional[Sequence[str]] = None,
) -> Tuple[Dict[str, Any], Dict[str, Any], List[str]]:
    """Collect persisted and full validation definitions for one cube export."""

    definitions: Dict[str, Any] = {}
    validation_definitions: Dict[str, Any] = {}
    warnings: List[str] = []
    class_types: List[str] = []
    seen_types: set[str] = set()

    for node_id in symbols:
        class_type = graph.nodes[node_id].class_type
        normalized = class_type.strip() if isinstance(class_type, str) else ""
        if (
            not normalized
            or is_subgraph_wrapper_type(normalized)
            or normalized in seen_types
        ):
            continue
        seen_types.add(normalized)
        class_types.append(normalized)

    if extra_class_types:
        for class_type in extra_class_types:
            normalized = class_type.strip() if isinstance(class_type, str) else ""
            if not normalized or normalized in seen_types:
                continue
            seen_types.add(normalized)
            class_types.append(normalized)

    for class_type in class_types:
        definition: Optional[Mapping[str, Any]] = None
        resolver_failed = False
        if resolver is not None:
            try:
                definition = resolver(class_type)
            except Exception as exc:  # pragma: no cover - resolver is optional
                warnings.append(f"Definition lookup failed for '{class_type}': {exc}")
                resolver_failed = True

        if not isinstance(definition, Mapping) and not resolver_failed:
            try:
                definition = resolve_definition_via_nodes(class_type)
            except Exception as exc:  # pragma: no cover - defensive
                warnings.append(
                    f"Definition introspection failed for '{class_type}': {exc}"
                )
                definition = None

        if not isinstance(definition, Mapping):
            warnings.append(f"No definition available for '{class_type}'")
            continue

        validation_definitions[class_type] = deepcopy(dict(definition))
        definitions[class_type] = _normalize_definition_map(class_type, definition)

    return definitions, validation_definitions, warnings


def collect_subgraph_node_types(subgraphs: Sequence[Mapping[str, Any]]) -> List[str]:
    """Collect non-wrapper class types referenced by serialized subgraph nodes."""

    class_types: List[str] = []
    seen_types: set[str] = set()
    for subgraph in subgraphs:
        nodes_payload = subgraph.get("nodes")
        if not isinstance(nodes_payload, Sequence):
            continue
        for node in nodes_payload:
            if not isinstance(node, Mapping):
                continue
            class_type = node.get("type")
            if not isinstance(class_type, str):
                class_type = node.get("class_type")
            normalized = class_type.strip() if isinstance(class_type, str) else ""
            if (
                not normalized
                or is_subgraph_wrapper_type(normalized)
                or normalized in seen_types
            ):
                continue
            seen_types.add(normalized)
            class_types.append(normalized)
    return class_types


def is_subgraph_wrapper_type(class_type: str) -> bool:
    """Return whether a class type is a Comfy subgraph wrapper UUID."""

    return bool(_UUID_RE.match(class_type.strip()))


def resolve_definition_via_nodes(class_type: str) -> Optional[Mapping[str, Any]]:
    """Resolve a node definition through the live Comfy registry when available."""

    nodes_module, comfy_node_internal_type = _load_comfy_runtime()
    if nodes_module is None or not hasattr(nodes_module, "NODE_CLASS_MAPPINGS"):
        return None

    mapping = getattr(nodes_module, "NODE_CLASS_MAPPINGS")
    if not isinstance(mapping, Mapping):
        return None

    obj_class = mapping.get(class_type)
    if obj_class is None:
        return None

    if (
        comfy_node_internal_type is not None
        and isinstance(obj_class, type)
        and issubclass(obj_class, comfy_node_internal_type)
    ):
        node_info_factory = getattr(obj_class, "GET_NODE_INFO_V1", None)
        if not callable(node_info_factory):
            return None
        runtime_info = node_info_factory()
        return dict(runtime_info) if isinstance(runtime_info, Mapping) else None

    input_types_factory = getattr(obj_class, "INPUT_TYPES", None)
    if callable(input_types_factory):
        input_types = input_types_factory()
    else:
        input_types = {}

    info: Dict[str, Any] = {}
    info["input"] = input_types
    if isinstance(input_types, Mapping):
        info["input_order"] = {
            key: list(value.keys())
            for key, value in input_types.items()
            if isinstance(value, Mapping)
        }
    else:
        info["input_order"] = {}

    outputs = getattr(obj_class, "RETURN_TYPES", ())
    info["output"] = list(outputs) if isinstance(outputs, (list, tuple)) else outputs

    output_is_list = getattr(obj_class, "OUTPUT_IS_LIST", None)
    if isinstance(output_is_list, (list, tuple)):
        info["output_is_list"] = list(output_is_list)
    elif isinstance(info["output"], list):
        info["output_is_list"] = [False] * len(info["output"])
    else:
        info["output_is_list"] = []

    output_names = getattr(obj_class, "RETURN_NAMES", None)
    if isinstance(output_names, (list, tuple)):
        info["output_name"] = list(output_names)
    else:
        info["output_name"] = info["output"]

    info["name"] = class_type
    display_map = getattr(nodes_module, "NODE_DISPLAY_NAME_MAPPINGS", {})
    if isinstance(display_map, Mapping) and class_type in display_map:
        info["display_name"] = display_map[class_type]
    else:
        info["display_name"] = class_type

    description = getattr(obj_class, "DESCRIPTION", "")
    info["description"] = description if isinstance(description, str) else ""

    info["python_module"] = getattr(
        obj_class, "RELATIVE_PYTHON_MODULE", getattr(obj_class, "__module__", "nodes")
    )
    info["category"] = getattr(obj_class, "CATEGORY", "sd")
    info["output_node"] = bool(getattr(obj_class, "OUTPUT_NODE", False))

    if getattr(obj_class, "DEPRECATED", False):
        info["deprecated"] = True
    if getattr(obj_class, "EXPERIMENTAL", False):
        info["experimental"] = True

    api_node = getattr(obj_class, "API_NODE", None)
    if api_node is not None:
        info["api_node"] = api_node

    return dict(info)


def _load_comfy_runtime() -> Tuple[Any, Any]:
    """Lazily load Comfy runtime modules used only for fallback introspection."""

    global _COMFY_RUNTIME_RESOLVED
    global _COMFY_NODES_MODULE
    global _COMFY_NODE_INTERNAL_TYPE

    if _COMFY_RUNTIME_RESOLVED:
        return _COMFY_NODES_MODULE, _COMFY_NODE_INTERNAL_TYPE

    try:
        _COMFY_NODES_MODULE = importlib.import_module("nodes")
    except (ImportError, ModuleNotFoundError):
        _COMFY_NODES_MODULE = None

    try:
        internal_module = importlib.import_module("comfy_api.internal")
    except (ImportError, ModuleNotFoundError):
        _COMFY_NODE_INTERNAL_TYPE = None
    else:
        _COMFY_NODE_INTERNAL_TYPE = getattr(internal_module, "_ComfyNodeInternal", None)

    _COMFY_RUNTIME_RESOLVED = True
    return _COMFY_NODES_MODULE, _COMFY_NODE_INTERNAL_TYPE


def _normalize_definition_map(
    class_type: str, definition: Mapping[str, Any]
) -> Dict[str, Any]:
    """Normalize a live node definition into compact JSON-friendly metadata."""

    normalized: Dict[str, Any] = {}
    for key, value in definition.items():
        if key in _DEFINITION_HELP_KEYS:
            continue
        if key == "input" and isinstance(value, Mapping):
            normalized[key] = _normalize_definition_inputs(class_type, value)
            continue
        normalized[key] = _sanitize_definition_value(value)
    return normalized


def _normalize_definition_inputs(
    class_type: str, sections: Mapping[str, Any]
) -> Dict[str, Any]:
    """Normalize node input declarations while stripping local inventory choices."""

    normalized: Dict[str, Any] = {}
    for section_name, raw_fields in sections.items():
        if not isinstance(raw_fields, Mapping):
            normalized[section_name] = _jsonify_definition_value(raw_fields)
            continue
        normalized[section_name] = {
            str(input_name): _normalize_definition_input_field(
                class_type, str(input_name), field_spec
            )
            for input_name, field_spec in raw_fields.items()
        }
    return normalized


def _normalize_definition_input_field(
    class_type: str, input_name: str, field_spec: Any
) -> Any:
    """Normalize one input field and replace choice inventories with a marker."""

    _ = class_type, input_name
    normalized = _sanitize_definition_value(field_spec)
    return compact_picker_field_spec(normalized)


def _sanitize_definition_value(value: Any) -> Any:
    """Return a JSON-ready definition value without UI help metadata."""

    if isinstance(value, Mapping):
        return {
            key: _sanitize_definition_value(val)
            for key, val in value.items()
            if key not in _DEFINITION_HELP_KEYS
        }
    if isinstance(value, tuple):
        return [_sanitize_definition_value(item) for item in value]
    if isinstance(value, list):
        return [_sanitize_definition_value(item) for item in value]
    return _jsonify_definition_value(value)


def _jsonify_definition_value(value: Any) -> Any:
    """Convert tuples and nested mappings into JSON-serializable containers."""

    if isinstance(value, Mapping):
        return {key: _jsonify_definition_value(val) for key, val in value.items()}
    if isinstance(value, tuple):
        return [_jsonify_definition_value(item) for item in value]
    if isinstance(value, list):
        return [_jsonify_definition_value(item) for item in value]
    return value

