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
"""Serialization utilities for SugarCubes exporter."""

from __future__ import annotations

import hashlib
import importlib
import math
import re
from copy import deepcopy
from dataclasses import dataclass
from typing import (
    Any,
    Callable,
    Dict,
    List,
    Mapping,
    MutableMapping,
    Optional,
    Sequence,
    Tuple,
)

try:
    from ..cube_model import (
        CubeIdentityError,
        build_cube_definition_key,
        derive_route_from_cube_id,
        migrate_legacy_payload,
        sanitize_authored_defaults_payload,
    )
    from ..cube_model.picker_fields import (
        compact_picker_field_spec,
        find_input_field_spec,
        is_picker_field_spec,
        picker_options,
        widget_input_names,
    )
    from ..instrumentation import log_event
except ImportError:
    from cube_model import (
        CubeIdentityError,
        build_cube_definition_key,
        derive_route_from_cube_id,
        migrate_legacy_payload,
        sanitize_authored_defaults_payload,
    )
    from cube_model.picker_fields import (
        compact_picker_field_spec,
        find_input_field_spec,
        is_picker_field_spec,
        picker_options,
        widget_input_names,
    )
    from instrumentation import log_event
from .graph import CubeAnalysis, CubeData, CubeMarker, Graph, GraphNode, Edge
from .versioning import resolve_input_type, resolve_output_type_by_slot
from .widget_validation import (
    serialized_widget_names,
    validate_serialized_widget_values,
    validate_subgraph_widget_values,
)

BindingResolver = Callable[[str], Mapping[str, Any]]
BINDING_SENTINEL = "@binding"

_DEFAULT_NODE_SIZE = [180.0, 60.0]
_DEFAULT_MARKER_SIZE = [140.0, 46.0]
_DEFAULT_CHROME_MARGIN_LEFT = 10.0
_DEFAULT_CHROME_MARGIN_RIGHT = 10.0
_DEFAULT_CHROME_MARGIN_BOTTOM = 10.0
_DEFAULT_CHROME_MARGIN_INNER_TOP = 26.0
_DEFAULT_CHROME_PADDING_X = 2.0
_DEFAULT_CHROME_PADDING_Y = 2.0
_DEFAULT_CHROME_PADDING_TOP_EXTRA = 0.0
_DEFAULT_CHROME_HEADER_HEIGHT = 32.0
_CONTROL_AFTER_GENERATE_VALUES = frozenset(
    {"fixed", "increment", "decrement", "randomize"}
)
_SOCKET_ONLY_INPUT_TYPES = frozenset(
    {
        "CLIP",
        "CONDITIONING",
        "CONDITIONING,CONDITIONING_BATCH",
        "DETECTOR_MODEL",
        "IMAGE",
        "LATENT",
        "MASK",
        "MODEL",
        "SAM_MODEL",
        "SEGS",
        "VAE",
    }
)
_SURFACE_VALUE_TYPES_BY_INPUT_TYPE = {
    "BOOLEAN": "boolean",
    "COMBO": "string",
    "FLOAT": "number",
    "INT": "number",
    "LIST": "string",
    "NUMBER": "number",
    "STRING": "string",
    "TEXT": "string",
}
_UUID_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", re.IGNORECASE
)

_LAYOUT_FLAG_KEYS = ("collapsed",)
_LAYOUT_STYLE_KEYS_STR = ("color", "bgcolor")
_CUBE_LAYOUT_DEFINITION_METADATA_KEYS = (
    "schema",
    "managed",
    "cube_id",
    "default_alias",
    "target_model",
    "cube_version",
    "cube_revision_ref",
    "cube_definition_key",
    "surface_signature",
    "icon",
)
_CUBE_LAYOUT_TEMPLATE_METADATA_KEYS = ("markers", "nodes", "bounds")
_DEFINITION_HELP_KEYS = frozenset({"tooltip", "output_tooltips", "description"})
_COMFY_RUNTIME_RESOLVED = False
_COMFY_NODES_MODULE: Any = None
_COMFY_NODE_INTERNAL_TYPE: Any = None


@dataclass
class ExportedCube:
    """Represent one serialized cube plus export-time warnings."""

    default_alias: str
    cube: Dict[str, Any]
    warnings: List[str]
    version_auto: bool = False


@dataclass
class WorkflowLayoutIndex:
    """Capture workflow layout data needed during cube serialization."""

    nodes: Dict[str, Mapping[str, Any]]
    ds: Dict[str, Any]
    groups: List[Dict[str, Any]]
    version: Optional[int]


def serialize(
    analysis: CubeAnalysis,
    *,
    workflow: Optional[Mapping[str, Any]] = None,
    workflow_version: Optional[int] = None,
    definition_resolver: Optional[BindingResolver] = None,
    cube_ids: Optional[Sequence[str]] = None,
) -> List[ExportedCube]:
    """Serialize analyzed cubes into `.cube` payloads."""

    results: List[ExportedCube] = []
    allowed_ids = {cube_id for cube_id in cube_ids if cube_id} if cube_ids else None
    workflow_ctx = _build_workflow_index(workflow) if workflow is not None else None
    subgraph_defs = _build_subgraph_index(workflow) if workflow else {}
    for cube in sorted(analysis.cubes.values(), key=lambda data: data.name):
        if allowed_ids is not None and cube.cube_id not in allowed_ids:
            continue
        results.append(
            _serialize_cube(
                analysis,
                cube,
                definition_resolver,
                workflow_ctx,
                workflow_version,
                subgraph_defs,
            )
        )
    return results


def _serialize_cube(
    analysis: CubeAnalysis,
    cube: CubeData,
    resolver: Optional[BindingResolver],
    layout_ctx: Optional["WorkflowLayoutIndex"],
    workflow_version: Optional[int],
    subgraph_defs: Mapping[str, Mapping[str, Any]],
) -> ExportedCube:
    """Serialize one analyzed cube into the persisted payload shape."""

    graph = analysis.graph
    symbols = _symbolize_nodes(cube, graph)
    subgraphs, subgraph_warnings = _collect_subgraphs(cube, graph, subgraph_defs)
    subgraph_class_types = _collect_subgraph_node_types(subgraphs)
    definitions, validation_definitions, definition_warnings = _collect_definitions(
        symbols, graph, resolver, extra_class_types=subgraph_class_types
    )
    validate_subgraph_widget_values(subgraphs, validation_definitions)
    inputs, alias_lookup, input_warnings = _build_inputs(
        cube, graph, symbols, definitions
    )
    outputs, output_alias_lookup, output_warnings = _build_outputs(
        cube, graph, symbols, definitions
    )
    nodes = _build_node_payloads(
        cube,
        graph,
        symbols,
        alias_lookup,
        layout_ctx,
        definitions,
        validation_definitions,
    )
    description, metadata = _describe_cube(cube)
    metadata, version_auto, cube_id, version = _ensure_metadata_defaults(
        cube, graph, metadata
    )
    default_alias = _derive_serialized_default_alias(cube, cube_id)
    metadata = _persist_default_alias_metadata(metadata, default_alias)
    layout_payload, layout_warnings = _build_layout_payload(
        cube,
        graph,
        symbols,
        alias_lookup,
        output_alias_lookup,
        layout_ctx,
        workflow_version,
        canonical_cube_id=cube_id,
        canonical_default_alias=default_alias,
        canonical_version=version,
    )

    legacy_payload: Dict[str, Any] = {
        "cube_id": cube_id,
        "version": version,
        "description": description,
        "nodes": nodes,
        "inputs": inputs,
    }
    if outputs:
        legacy_payload["outputs"] = outputs
    legacy_payload["definitions"] = definitions
    if metadata:
        legacy_payload["metadata"] = metadata
    if layout_payload:
        legacy_payload["layout"] = layout_payload
    if subgraphs:
        legacy_payload["subgraphs"] = subgraphs

    payload = migrate_legacy_payload(legacy_payload).to_dict()
    sanitize_authored_defaults_payload(payload)
    _validate_authored_values_against_definitions(payload)

    warnings = (
        input_warnings
        + output_warnings
        + definition_warnings
        + subgraph_warnings
        + layout_warnings
    )

    implementation_layout = payload.get("implementation", {}).get("layout", {})
    if implementation_layout:
        layout_summary = {
            "present": True,
            "node_entries": len(implementation_layout.get("nodes", {})),
            "marker_entries": len(implementation_layout.get("markers", {})),
            "groups": len(implementation_layout.get("groups", [])),
        }
    else:
        layout_summary = {
            "present": False,
            "node_entries": 0,
            "marker_entries": 0,
            "groups": 0,
        }

    log_event(
        "exporter.phase2",
        "serialize_cube",
        {
            "cube": cube.name,
            "node_count": len(nodes),
            "marker_counts": {
                "inputs": len(cube.inputs),
                "outputs": len(cube.outputs),
            },
            "layout": layout_summary,
            "metadata_present": bool(metadata),
            "warnings": warnings,
        },
    )
    return ExportedCube(
        default_alias=cube.name,
        cube=payload,
        warnings=warnings,
        version_auto=version_auto,
    )


def _symbolize_nodes(cube: CubeData, graph: Graph) -> Dict[str, str]:
    """Assign stable symbol names to each executable node in the cube."""

    symbols: Dict[str, str] = {}
    used: Dict[str, int] = {}
    for node_id in sorted(cube.subgraph_nodes, key=_natural_key):
        base = _symbol_base(graph.nodes[node_id])
        symbols[node_id] = _dedupe(base, used)
    return symbols


def _build_inputs(
    cube: CubeData,
    graph: Graph,
    symbols: Mapping[str, str],
    definitions: Mapping[str, Any],
) -> Tuple[Dict[str, Any], Dict[str, str], List[str]]:
    """Build serialized input bindings for the cube payload."""

    inputs: Dict[str, Any] = {}
    alias_lookup: Dict[str, str] = {}
    warnings: List[str] = []
    counters: Dict[Tuple[str, str], int] = {}

    for marker in sorted(cube.inputs, key=lambda entry: _natural_key(entry.node_id)):
        edges = _downstream_edges(marker, graph, cube.subgraph_nodes)
        connections = _downstream_connections(edges, symbols)
        binding_type = _resolve_input_binding_type(edges, graph, definitions)
        alias = _make_binding_key("input", binding_type, counters)
        alias_lookup[marker.node_id] = alias
        inputs[alias] = {
            "kind": "input",
            "targets": connections,
        }
        if not connections:
            warnings.append(
                f"CubeInput '{marker.node_id}' has no downstream connections"
            )

    return inputs, alias_lookup, warnings


def _build_outputs(
    cube: CubeData,
    graph: Graph,
    symbols: Mapping[str, str],
    definitions: Mapping[str, Any],
) -> Tuple[Dict[str, Any], Dict[str, str], List[str]]:
    """Build serialized output bindings for the cube payload."""

    outputs: Dict[str, Any] = {}
    alias_lookup: Dict[str, str] = {}
    warnings: List[str] = []
    counters: Dict[Tuple[str, str], int] = {}

    for marker in sorted(cube.outputs, key=lambda entry: _natural_key(entry.node_id)):
        upstream = _upstream_edges(marker, graph, cube.subgraph_nodes)
        if not upstream:
            warnings.append(f"CubeOutput '{marker.node_id}' has no upstream source")
            continue
        if len(upstream) > 1:
            warnings.append(
                f"CubeOutput '{marker.node_id}' has multiple upstream sources; taking the first"
            )
        edge = upstream[0]
        symbol = symbols[edge.source]
        binding_type = _resolve_output_binding_type(edge, graph, definitions)
        alias = _make_binding_key("output", binding_type, counters)
        outputs[alias] = symbol
        alias_lookup[marker.node_id] = alias

    return outputs, alias_lookup, warnings


def _build_node_payloads(
    cube: CubeData,
    graph: Graph,
    symbols: Mapping[str, str],
    alias_lookup: Mapping[str, str],
    layout_ctx: Optional["WorkflowLayoutIndex"],
    definitions: Mapping[str, Any],
    validation_definitions: Mapping[str, Any],
) -> Dict[str, Any]:
    """Build the serialized node map with binding-aware input remapping."""

    nodes: Dict[str, Any] = {}
    for node_id in sorted(cube.subgraph_nodes, key=_natural_key):
        node = graph.nodes[node_id]
        payload_inputs: Dict[str, Any] = {}
        for key, value in node.inputs.items():
            payload_inputs[key] = _remap_value(value, symbols, alias_lookup)
        _backfill_workflow_widget_inputs(
            payload_inputs,
            node,
            layout_ctx.nodes.get(node_id) if layout_ctx else None,
            validation_definitions,
        )
        node_payload: Dict[str, Any] = {
            "class_type": node.class_type,
            "label": _resolve_node_label(
                symbols[node_id],
                layout_ctx.nodes.get(node_id) if layout_ctx else None,
                node,
            ),
            "inputs": payload_inputs,
            "original_id": node_id,
        }
        workflow_node = layout_ctx.nodes.get(node_id) if layout_ctx else None
        mode = _extract_execution_mode(workflow_node if workflow_node else node.data)
        if mode is not None:
            node_payload["mode"] = mode
        nodes[symbols[node_id]] = node_payload
    return nodes


def _backfill_workflow_widget_inputs(
    payload_inputs: MutableMapping[str, Any],
    node: GraphNode,
    workflow_node: Optional[Mapping[str, Any]],
    definitions: Mapping[str, Any],
) -> None:
    """Recover widget values omitted from prompt inputs using workflow metadata."""

    if not isinstance(workflow_node, Mapping):
        return
    widget_values = workflow_node.get("widgets_values")
    if not isinstance(widget_values, Sequence) or isinstance(
        widget_values, (str, bytes)
    ):
        return
    definition = definitions.get(node.class_type)
    if not isinstance(definition, Mapping):
        return
    persisted_names = serialized_widget_names(workflow_node)
    if not persisted_names:
        persisted_names = widget_input_names(definition)
    validate_serialized_widget_values(
        node_id=node.id,
        class_type=node.class_type,
        persisted_widget_names=persisted_names,
        widget_values=widget_values,
        live_definition=definition,
    )
    value_index = 0
    for input_name in widget_input_names(definition):
        if value_index >= len(widget_values):
            return
        field_spec = find_input_field_spec(definition, input_name)
        value = widget_values[value_index]
        if input_name in payload_inputs:
            value_index = _advance_widget_value_index(
                widget_values, value_index, field_spec
            )
            continue
        if _is_control_after_generate_value(value):
            if _field_has_serialized_control_widget(field_spec):
                value_index += 1
                continue
            _raise_widget_backfill_error(
                node=node,
                input_name=input_name,
                field_spec=field_spec,
                value=value,
                reason="control-after-generate value cannot populate this input",
            )
        compatible, coerced_value = _coerce_widget_backfill_value(value, field_spec)
        if not compatible:
            _raise_widget_backfill_error(
                node=node,
                input_name=input_name,
                field_spec=field_spec,
                value=value,
                reason="workflow widget value is incompatible with input definition",
            )
        payload_inputs[input_name] = _jsonify_definition_value(coerced_value)
        value_index += 1
        if _field_has_serialized_control_widget(field_spec) and value_index < len(
            widget_values
        ):
            if _is_control_after_generate_value(widget_values[value_index]):
                value_index += 1


def _advance_widget_value_index(
    widget_values: Sequence[Any],
    value_index: int,
    field_spec: Any,
) -> int:
    """Advance over a serialized widget value and its optional control companion."""

    next_index = value_index + 1
    if _field_has_serialized_control_widget(field_spec) and next_index < len(
        widget_values
    ):
        if _is_control_after_generate_value(widget_values[next_index]):
            return next_index + 1
    return next_index


def _coerce_widget_backfill_value(value: Any, field_spec: Any) -> Tuple[bool, Any]:
    """Return whether a workflow widget value matches a definition field."""

    input_type = _field_type_name(field_spec)
    if input_type is None:
        return True, _jsonify_definition_value(value)
    normalized_type = input_type.upper()
    if normalized_type in _SOCKET_ONLY_INPUT_TYPES:
        return False, None
    if is_picker_field_spec(field_spec):
        return _coerce_picker_widget_value(value, field_spec)
    if normalized_type in {"STRING", "TEXT"}:
        return (True, value) if isinstance(value, str) else (False, None)
    if normalized_type == "BOOLEAN":
        return (True, value) if isinstance(value, bool) else (False, None)
    if normalized_type == "INT":
        if isinstance(value, bool):
            return False, None
        coerced = _coerce_int_value(value)
        return (True, coerced) if coerced is not None else (False, None)
    if normalized_type in {"FLOAT", "NUMBER"}:
        coerced_float = _coerce_widget_float(value)
        return (True, coerced_float) if coerced_float is not None else (False, None)
    return True, _jsonify_definition_value(value)


def _coerce_picker_widget_value(value: Any, field_spec: Any) -> Tuple[bool, Any]:
    """Return whether a picker widget value is safe to persist by name."""

    if not isinstance(value, str):
        return False, None
    options = picker_options(field_spec)
    if options and value not in options:
        return False, None
    return True, value


def _coerce_widget_float(value: Any) -> Optional[float]:
    """Coerce a workflow widget value to float only when conversion is safe."""

    if value is None or isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        stripped = value.strip()
        if not stripped:
            return None
        try:
            return float(stripped)
        except ValueError:
            return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _field_type_name(field_spec: Any) -> Optional[str]:
    """Return the declared scalar input type for a field spec."""

    if isinstance(field_spec, str):
        return field_spec
    if not isinstance(field_spec, Sequence) or isinstance(field_spec, (str, bytes)):
        return None
    if not field_spec:
        return None
    first = field_spec[0]
    return first if isinstance(first, str) else None


def _field_metadata(field_spec: Any) -> Optional[Mapping[str, Any]]:
    """Return the metadata mapping for a field spec when present."""

    if (
        isinstance(field_spec, Sequence)
        and not isinstance(field_spec, (str, bytes))
        and len(field_spec) > 1
        and isinstance(field_spec[1], Mapping)
    ):
        return field_spec[1]
    return None


def _field_has_serialized_control_widget(field_spec: Any) -> bool:
    """Return whether Comfy serializes a control value after this widget."""

    metadata = _field_metadata(field_spec)
    return bool(metadata and metadata.get("control_after_generate") is True)


def _is_control_after_generate_value(value: Any) -> bool:
    """Return whether a value is Comfy seed control metadata."""

    return (
        isinstance(value, str)
        and value.strip().lower() in _CONTROL_AFTER_GENERATE_VALUES
    )


def _raise_widget_backfill_error(
    *,
    node: GraphNode,
    input_name: str,
    field_spec: Any,
    value: Any,
    reason: str,
) -> None:
    """Raise a diagnostic error for unsafe positional widget backfill."""

    expected = _field_type_name(field_spec) or "unknown"
    raise ValueError(
        "Unsafe workflow widget backfill: "
        f"{reason}; node_id={node.id}; class_type={node.class_type}; "
        f"input={input_name}; expected={expected}; "
        f"actual_type={type(value).__name__}; actual_value={value!r}"
    )


def _validate_authored_values_against_definitions(payload: Mapping[str, Any]) -> None:
    """Reject cube payloads with surface values that contradict definitions."""

    implementation = payload.get("implementation")
    surface = payload.get("surface")
    flavors = payload.get("flavors")
    if not (
        isinstance(implementation, Mapping)
        and isinstance(surface, Mapping)
        and isinstance(flavors, Mapping)
    ):
        return
    nodes = implementation.get("nodes")
    definitions = implementation.get("definitions")
    controls = surface.get("controls")
    authored = flavors.get("authored")
    if not (
        isinstance(nodes, Mapping)
        and isinstance(definitions, Mapping)
        and isinstance(controls, Sequence)
        and not isinstance(controls, (str, bytes))
        and isinstance(authored, Sequence)
        and not isinstance(authored, (str, bytes))
    ):
        return
    control_index = {
        control.get("control_id"): control
        for control in controls
        if isinstance(control, Mapping) and isinstance(control.get("control_id"), str)
    }
    for control in control_index.values():
        _validate_surface_control_type(payload, control, definitions)
    for flavor in authored:
        if not isinstance(flavor, Mapping):
            continue
        values = flavor.get("values")
        if not isinstance(values, Mapping):
            continue
        flavor_id = str(flavor.get("id", ""))
        for control_id, value in values.items():
            control = control_index.get(control_id)
            if control is None:
                continue
            _validate_authored_control_value(
                payload,
                control,
                definitions,
                flavor_id=flavor_id,
                value=value,
            )


def _validate_surface_control_type(
    payload: Mapping[str, Any],
    control: Mapping[str, Any],
    definitions: Mapping[str, Any],
) -> None:
    """Reject a surface control whose coarse value type mismatches its input."""

    field_spec = _control_field_spec(control, definitions)
    if field_spec is None:
        return
    expected_value_type = _expected_surface_value_type(field_spec)
    if expected_value_type is None:
        return
    actual_value_type = control.get("value_type")
    if actual_value_type != expected_value_type:
        _raise_authored_value_error(
            payload=payload,
            control=control,
            field_spec=field_spec,
            flavor_id="",
            value=actual_value_type,
            reason=(
                "surface control value_type does not match node definition "
                f"(expected {expected_value_type!r})"
            ),
        )


def _validate_authored_control_value(
    payload: Mapping[str, Any],
    control: Mapping[str, Any],
    definitions: Mapping[str, Any],
    *,
    flavor_id: str,
    value: Any,
) -> None:
    """Reject an authored value that cannot be applied to its definition field."""

    field_spec = _control_field_spec(control, definitions)
    if field_spec is None:
        return
    compatible, _coerced = _coerce_widget_backfill_value(value, field_spec)
    if compatible:
        return
    _raise_authored_value_error(
        payload=payload,
        control=control,
        field_spec=field_spec,
        flavor_id=flavor_id,
        value=value,
        reason="authored default does not match node definition",
    )


def _control_field_spec(
    control: Mapping[str, Any],
    definitions: Mapping[str, Any],
) -> Any | None:
    """Return the embedded definition field spec for one surface control."""

    class_type = control.get("class_type")
    input_name = control.get("input_name")
    if not isinstance(class_type, str) or not isinstance(input_name, str):
        return None
    definition = definitions.get(class_type)
    if not isinstance(definition, Mapping):
        return None
    return find_input_field_spec(definition, input_name)


def _expected_surface_value_type(field_spec: Any) -> Optional[str]:
    """Return the coarse surface value type implied by a definition field."""

    if is_picker_field_spec(field_spec):
        return "string"
    input_type = _field_type_name(field_spec)
    if input_type is None:
        return None
    return _SURFACE_VALUE_TYPES_BY_INPUT_TYPE.get(input_type.upper())


def _raise_authored_value_error(
    *,
    payload: Mapping[str, Any],
    control: Mapping[str, Any],
    field_spec: Any,
    flavor_id: str,
    value: Any,
    reason: str,
) -> None:
    """Raise a diagnostic error for a self-inconsistent cube surface value."""

    cube_id = payload.get("cube_id", "")
    control_id = control.get("control_id", "")
    symbol = control.get("symbol", "")
    class_type = control.get("class_type", "")
    input_name = control.get("input_name", "")
    expected = _field_type_name(field_spec) or "unknown"
    raise ValueError(
        "Cube authored default does not match node definition: "
        f"{reason}; cube_id={cube_id}; flavor={flavor_id}; "
        f"control_id={control_id}; symbol={symbol}; class_type={class_type}; "
        f"input={input_name}; expected={expected}; "
        f"actual_type={type(value).__name__}; actual_value={value!r}"
    )


def _collect_definitions(
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
            or _is_subgraph_wrapper_type(normalized)
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
                definition = _resolve_definition_via_nodes(class_type)
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


def _collect_subgraph_node_types(subgraphs: Sequence[Mapping[str, Any]]) -> List[str]:
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
                or _is_subgraph_wrapper_type(normalized)
                or normalized in seen_types
            ):
                continue
            seen_types.add(normalized)
            class_types.append(normalized)
    return class_types


def _is_subgraph_wrapper_type(class_type: str) -> bool:
    """Return whether a class type is a Comfy subgraph wrapper UUID."""

    return bool(_UUID_RE.match(class_type.strip()))


def _resolve_definition_via_nodes(class_type: str) -> Optional[Mapping[str, Any]]:
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
        info = obj_class.GET_NODE_INFO_V1()
        return info if isinstance(info, Mapping) else None

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

    return info


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


def _describe_cube(cube: CubeData) -> Tuple[str, Dict[str, Any]]:
    """Return default authored metadata for a newly exported cube."""

    return f"Auto-converted cube for {cube.name.lower()}", {}


def _ensure_metadata_defaults(
    cube: CubeData, graph: Graph, metadata: Dict[str, Any]
) -> Tuple[Dict[str, Any], bool, str, str]:
    """Apply default cube id and version values while validating overrides."""

    cube_id = cube.cube_id.strip() if isinstance(cube.cube_id, str) else ""
    if not cube_id:
        slug = _sanitize_identifier(cube.name).replace("_", "-")
        fingerprint = _cube_fingerprint(cube, graph)
        suffix = _short_hash(fingerprint)
        cube_id = f"{slug}-{suffix}" if suffix else slug

    metadata_id = metadata.get("id")
    if isinstance(metadata_id, str) and metadata_id.strip():
        if metadata_id.strip() != cube_id:
            raise ValueError(
                f"Cube id mismatch for '{cube.name}': '{metadata_id.strip()}' vs '{cube_id}'"
            )
        metadata.pop("id", None)

    version_value = metadata.get("version")
    version_auto = not isinstance(version_value, str) or not version_value.strip()
    if version_auto:
        version = "1.0.0"
    else:
        version = version_value.strip()
    metadata.pop("version", None)
    return metadata, version_auto, cube_id, version


def _derive_serialized_default_alias(cube: CubeData, cube_id: str) -> str:
    """Return the route alias that should be persisted for one exported cube."""

    try:
        return derive_route_from_cube_id(cube_id)
    except CubeIdentityError:
        return cube.name.strip()


def _persist_default_alias_metadata(
    metadata: Dict[str, Any], default_alias: str
) -> Dict[str, Any]:
    """Persist the canonical cube default alias into serialized metadata."""

    if default_alias:
        metadata["default_alias"] = default_alias
    return metadata


def _cube_fingerprint(cube: CubeData, graph: Graph) -> str:
    """Build a stable fallback fingerprint for generated cube ids."""

    class_types: List[str] = []
    for node_id in cube.subgraph_nodes:
        node = graph.nodes.get(node_id)
        if not node:
            continue
        class_types.append(node.class_type)
    class_types.sort()
    return f"{cube.name}|{','.join(class_types)}"


def _short_hash(value: str) -> str:
    """Return a short deterministic hash suffix for generated identifiers."""

    if not value:
        return ""
    digest = hashlib.sha1(value.encode("utf-8")).hexdigest()
    return digest[:6]


def _build_workflow_index(
    workflow: Optional[Mapping[str, Any]],
) -> Optional["WorkflowLayoutIndex"]:
    """Index workflow nodes and layout metadata for later layout serialization."""

    if workflow is None or not isinstance(workflow, Mapping):
        return None
    nodes: Dict[str, Mapping[str, Any]] = {}
    node_list = workflow.get("nodes")
    if isinstance(node_list, Sequence):
        for node in node_list:
            if not isinstance(node, Mapping):
                continue
            node_id = node.get("id")
            if node_id is None:
                continue
            nodes[str(node_id)] = node
    extra = workflow.get("extra")
    ds_value = extra.get("ds") if isinstance(extra, Mapping) else None
    ds = _coerce_layout_ds(ds_value)
    groups: List[Dict[str, Any]] = []
    group_list = workflow.get("groups")
    if isinstance(group_list, Sequence):
        for group in group_list:
            if isinstance(group, Mapping):
                groups.append(dict(group))
    version = _coerce_int_value(workflow.get("version"))
    return WorkflowLayoutIndex(nodes=nodes, ds=ds, groups=groups, version=version)


def _build_subgraph_index(
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


def _collect_subgraphs(
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
        elif _is_subgraph_wrapper_type(class_type):
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
            or not _is_subgraph_wrapper_type(normalized)
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


def _build_layout_payload(
    cube: CubeData,
    graph: Graph,
    symbols: Mapping[str, str],
    alias_lookup: Mapping[str, str],
    output_alias_lookup: Mapping[str, str],
    layout_ctx: Optional["WorkflowLayoutIndex"],
    workflow_version: Optional[int],
    *,
    canonical_cube_id: str,
    canonical_default_alias: str,
    canonical_version: str,
) -> Tuple[Optional[Dict[str, Any]], List[str]]:
    """Serialize workflow layout metadata for nodes, markers, and groups."""

    warnings: List[str] = []
    if layout_ctx is None:
        warnings.append(
            "Layout metadata unavailable; frontend did not provide workflow data"
        )
        return None, warnings

    marker_ids = cube.marker_ids()
    node_entries: Dict[str, Dict[str, Any]] = {}
    marker_entries: Dict[str, Dict[str, Any]] = {}
    positions: List[List[float]] = []

    for node_id in sorted(cube.subgraph_nodes, key=_natural_key):
        if node_id in marker_ids:
            continue
        workflow_node = layout_ctx.nodes.get(node_id)
        if not isinstance(workflow_node, Mapping):
            warnings.append(f"Node '{node_id}' missing from workflow layout")
            continue
        pos = _coerce_vec2(workflow_node.get("pos"))
        if pos is None:
            warnings.append(f"Node '{node_id}' missing position in workflow layout")
            continue
        graph_node = graph.nodes.get(node_id)
        if not graph_node:
            continue
        size = _coerce_vec2(workflow_node.get("size"))
        if size is None:
            warnings.append(
                f"Node '{node_id}' missing size in workflow layout; using default"
            )
            size = list(_DEFAULT_NODE_SIZE)
        entry: Dict[str, Any] = {
            "id": node_id,
            "class_type": workflow_node.get("type") or graph_node.class_type,
            "pos": pos,
            "size": size,
        }
        title = _resolve_layout_title(workflow_node, graph_node)
        if title:
            entry["title"] = title
        flags = _extract_layout_flags(workflow_node)
        if flags:
            entry["flags"] = flags
        style = _extract_layout_style(workflow_node)
        if style:
            entry["style"] = style
        node_entries[symbols[node_id]] = entry
        positions.append(pos)

    def add_marker_entries(
        markers: Sequence[CubeMarker], alias_map: Mapping[str, str]
    ) -> None:
        for marker in markers:
            alias = alias_map.get(marker.node_id)
            if not alias:
                warnings.append(
                    f"Marker '{marker.node_id}' ({marker.kind}) missing binding alias; layout entry skipped"
                )
                continue
            workflow_node = layout_ctx.nodes.get(marker.node_id)
            if not isinstance(workflow_node, Mapping):
                warnings.append(
                    f"Marker '{marker.node_id}' missing from workflow layout"
                )
                continue
            pos = _coerce_vec2(workflow_node.get("pos"))
            if pos is None:
                warnings.append(
                    f"Marker '{marker.node_id}' missing position in workflow layout"
                )
                continue
            graph_node = graph.nodes.get(marker.node_id)
            if not graph_node:
                continue
            size = _coerce_vec2(workflow_node.get("size"))
            if size is None:
                warnings.append(
                    f"Marker '{marker.node_id}' missing size in workflow layout; using default"
                )
                size = list(_DEFAULT_MARKER_SIZE)
            entry: Dict[str, Any] = {
                "id": marker.node_id,
                "class_type": workflow_node.get("type") or graph_node.class_type,
                "kind": marker.kind,
                "pos": pos,
                "size": size,
            }
            title = _resolve_layout_title(workflow_node, graph_node)
            if title:
                entry["title"] = title
            flags = _extract_layout_flags(workflow_node)
            if flags:
                entry["flags"] = flags
            style = _extract_layout_style(workflow_node)
            if style:
                entry["style"] = style
            marker_entries[alias] = entry
            positions.append(pos)

    add_marker_entries(cube.inputs, alias_lookup)
    add_marker_entries(cube.outputs, output_alias_lookup)

    if not node_entries and not marker_entries:
        warnings.append(f"No layout entries recorded for cube '{cube.name}'")
        return None, warnings

    origin = _compute_layout_origin(positions)
    _normalize_entry_positions(node_entries, origin)
    _normalize_entry_positions(marker_entries, origin)
    groups = _normalize_layout_groups(
        layout_ctx.groups,
        origin,
        node_entries,
        marker_entries,
        canonical_cube_id=canonical_cube_id,
        canonical_default_alias=canonical_default_alias,
        canonical_version=canonical_version,
        canonical_name_locked=cube.name_from_lookup,
    )

    effective_version = (
        workflow_version if workflow_version is not None else layout_ctx.version
    )
    if effective_version is None:
        effective_version = 0

    layout_payload: Dict[str, Any] = {
        "workflow_version": int(effective_version),
        "origin": origin,
        "ds": _copy_layout_ds(layout_ctx.ds),
        "nodes": node_entries,
    }
    if marker_entries:
        layout_payload["markers"] = marker_entries
    if groups:
        layout_payload["groups"] = groups

    return layout_payload, warnings


def _resolve_layout_title(
    workflow_node: Mapping[str, Any], graph_node: Optional[GraphNode]
) -> Optional[str]:
    """Resolve the title persisted for one layout entry."""

    title = workflow_node.get("title")
    if isinstance(title, str) and title.strip():
        return title
    if graph_node:
        meta_title = graph_node.meta.get("title")
        if isinstance(meta_title, str) and meta_title.strip():
            return meta_title
    return None


def _resolve_node_label(
    symbol: str,
    workflow_node: Optional[Mapping[str, Any]],
    graph_node: GraphNode,
) -> str:
    """Resolve the script-facing node label stored in implementation nodes."""

    if isinstance(workflow_node, Mapping):
        title = workflow_node.get("title")
        if isinstance(title, str) and title.strip():
            return title.strip()
    meta_title = graph_node.meta.get("title")
    if isinstance(meta_title, str) and meta_title.strip():
        return meta_title.strip()
    return symbol


def _compute_layout_origin(positions: Sequence[Sequence[float]]) -> List[float]:
    """Compute the top-left origin used to normalize layout coordinates."""

    if not positions:
        return [_round_value(0.0), _round_value(0.0)]
    min_x = min(pos[0] for pos in positions)
    min_y = min(pos[1] for pos in positions)
    return [_round_value(min_x), _round_value(min_y)]


def _normalize_entry_positions(
    entries: Mapping[str, Dict[str, Any]], origin: Sequence[float]
) -> None:
    """Rewrite entry positions relative to the computed layout origin."""

    ox, oy = origin
    for entry in entries.values():
        pos = entry.get("pos")
        if pos is None:
            continue
        entry["pos"] = [_round_value(pos[0] - ox), _round_value(pos[1] - oy)]


def _normalize_layout_groups(
    groups: Sequence[Mapping[str, Any]],
    origin: Sequence[float],
    node_entries: Mapping[str, Dict[str, Any]],
    marker_entries: Mapping[str, Dict[str, Any]],
    *,
    canonical_cube_id: str,
    canonical_default_alias: str,
    canonical_version: str,
    canonical_name_locked: bool,
) -> List[Dict[str, Any]]:
    """Normalize persisted groups and drop ones that no longer bound content."""

    if not groups:
        return []

    node_positions: List[Sequence[float]] = []
    for entry in node_entries.values():
        pos = entry.get("pos")
        if isinstance(pos, (list, tuple)) and len(pos) == 2:
            node_positions.append(pos)
    for entry in marker_entries.values():
        pos = entry.get("pos")
        if isinstance(pos, (list, tuple)) and len(pos) == 2:
            node_positions.append(pos)

    normalized: List[Dict[str, Any]] = []
    for group in groups:
        data = dict(group)
        bounding = _coerce_vec4(group.get("bounding"))
        include = True
        if bounding:
            bounding[0] = _round_value(bounding[0] - origin[0])
            bounding[1] = _round_value(bounding[1] - origin[1])
            data["bounding"] = bounding
            if node_positions:
                bx, by, bw, bh = bounding
                include = any(
                    bx <= pos[0] <= bx + bw and by <= pos[1] <= by + bh
                    for pos in node_positions
                )
        sugarcubes = data.get("sugarcubes")
        if isinstance(sugarcubes, Mapping):
            cleaned = _sanitize_cube_layout_group_metadata(
                sugarcubes,
                canonical_cube_id=canonical_cube_id,
                canonical_default_alias=canonical_default_alias,
                canonical_version=canonical_version,
                canonical_name_locked=canonical_name_locked,
            )
            chrome = _derive_reusable_group_chrome(
                origin,
                node_entries,
                marker_entries,
                cleaned,
            )
            if chrome and _group_targets_cube(cleaned, canonical_cube_id):
                data["bounding"] = chrome["bounding"]
                cleaned["bounds"] = chrome["bounds"]
            if (
                canonical_name_locked
                and _group_targets_cube(cleaned, canonical_cube_id)
                and canonical_default_alias
            ):
                data["title"] = canonical_default_alias
            data["sugarcubes"] = cleaned
        if include:
            normalized.append(data)
    return normalized


def _derive_reusable_group_chrome(
    origin: Sequence[float],
    node_entries: Mapping[str, Dict[str, Any]],
    marker_entries: Mapping[str, Dict[str, Any]],
    metadata: Mapping[str, Any],
) -> Optional[Dict[str, Any]]:
    """Derive reusable cube chrome geometry from normalized content layout."""

    content = _compute_layout_content_bounds(node_entries, marker_entries)
    if content is None:
        return None
    padding = _resolve_chrome_padding(metadata.get("bounds"))
    header = _resolve_chrome_header(metadata.get("bounds"))
    min_x, min_y, max_x, max_y = content
    top_margin = (
        padding["y"]
        + padding["top_extra"]
        + header["height"]
        + _DEFAULT_CHROME_MARGIN_INNER_TOP
    )
    rel_x = min_x - _DEFAULT_CHROME_MARGIN_LEFT
    rel_y = min_y - top_margin
    rel_w = max_x - min_x + _DEFAULT_CHROME_MARGIN_LEFT + _DEFAULT_CHROME_MARGIN_RIGHT
    rel_h = max_y - min_y + top_margin + _DEFAULT_CHROME_MARGIN_BOTTOM
    bounding = [
        _round_value(rel_x),
        _round_value(rel_y),
        _round_value(rel_w),
        _round_value(rel_h),
    ]
    bounds = {
        "x": _round_value(origin[0] + rel_x),
        "y": _round_value(origin[1] + rel_y),
        "w": _round_value(rel_w),
        "h": _round_value(rel_h),
        "padding": {
            "x": _round_value(padding["x"]),
            "y": _round_value(padding["y"]),
            "top_extra": _round_value(padding["top_extra"]),
        },
        "header": {"height": _round_value(header["height"])},
    }
    return {"bounding": bounding, "bounds": bounds}


def _compute_layout_content_bounds(
    node_entries: Mapping[str, Dict[str, Any]],
    marker_entries: Mapping[str, Dict[str, Any]],
) -> Optional[Tuple[float, float, float, float]]:
    """Compute normalized layout content bounds for reusable chrome."""

    min_x = float("inf")
    min_y = float("inf")
    max_x = float("-inf")
    max_y = float("-inf")
    for entry in list(node_entries.values()) + list(marker_entries.values()):
        pos = _coerce_vec2(entry.get("pos"))
        size = _coerce_vec2(entry.get("size"))
        if pos is None or size is None:
            continue
        width = max(0.0, size[0])
        height = max(0.0, size[1])
        min_x = min(min_x, pos[0])
        min_y = min(min_y, pos[1])
        max_x = max(max_x, pos[0] + width)
        max_y = max(max_y, pos[1] + height)
    if not all(map(_is_finite_number, (min_x, min_y, max_x, max_y))):
        return None
    return min_x, min_y, max_x, max_y


def _resolve_chrome_padding(bounds: Any) -> Dict[str, float]:
    """Resolve reusable chrome padding metadata from saved bounds."""

    padding = bounds.get("padding") if isinstance(bounds, Mapping) else None
    if not isinstance(padding, Mapping):
        padding = {}
    return {
        "x": _coerce_float(padding.get("x"), _DEFAULT_CHROME_PADDING_X),
        "y": _coerce_float(padding.get("y"), _DEFAULT_CHROME_PADDING_Y),
        "top_extra": _coerce_float(
            padding.get("top_extra"), _DEFAULT_CHROME_PADDING_TOP_EXTRA
        ),
    }


def _resolve_chrome_header(bounds: Any) -> Dict[str, float]:
    """Resolve reusable chrome header metadata from saved bounds."""

    header = bounds.get("header") if isinstance(bounds, Mapping) else None
    if not isinstance(header, Mapping):
        header = {}
    return {
        "height": _coerce_float(header.get("height"), _DEFAULT_CHROME_HEADER_HEIGHT)
    }


def _is_finite_number(value: float) -> bool:
    """Return whether a layout number is finite."""

    return math.isfinite(value)


def _sanitize_cube_layout_group_metadata(
    metadata: Mapping[str, Any],
    *,
    canonical_cube_id: str,
    canonical_default_alias: str,
    canonical_version: str,
    canonical_name_locked: bool,
) -> Dict[str, Any]:
    """Return definition-safe SugarCubes metadata for persisted cube layout."""

    definition = metadata.get("definition")
    if not isinstance(definition, Mapping):
        definition = {}
    instance = metadata.get("instance")
    if not isinstance(instance, Mapping):
        instance = {}

    cleaned: Dict[str, Any] = {}
    for key in _CUBE_LAYOUT_DEFINITION_METADATA_KEYS:
        value = definition.get(key) if key in definition else metadata.get(key)
        if _is_exportable_metadata_value(value):
            cleaned[key] = deepcopy(value)

    for key in _CUBE_LAYOUT_TEMPLATE_METADATA_KEYS:
        value = instance.get(key) if key in instance else metadata.get(key)
        if _is_exportable_metadata_value(value):
            cleaned[key] = deepcopy(value)

    if (
        canonical_name_locked
        and _group_targets_cube(cleaned, canonical_cube_id)
        and canonical_default_alias
    ):
        cleaned["default_alias"] = canonical_default_alias
    if _group_targets_cube(cleaned, canonical_cube_id):
        cleaned["cube_id"] = canonical_cube_id
        cleaned["cube_version"] = canonical_version
        cleaned["cube_definition_key"] = build_cube_definition_key(
            canonical_cube_id, canonical_version
        )
    return cleaned


def _is_exportable_metadata_value(value: Any) -> bool:
    """Return whether a metadata value should be retained in cube layout."""

    if value is None or value == "":
        return False
    if isinstance(value, (list, tuple, dict)) and len(value) == 0:
        return False
    return True


def _group_targets_cube(group_metadata: Mapping[str, Any], cube_id: str) -> bool:
    """Return whether the persisted group metadata belongs to the target cube."""

    if not cube_id:
        return False
    raw_cube_id = group_metadata.get("cube_id")
    if not isinstance(raw_cube_id, str):
        return False
    return raw_cube_id.strip() == cube_id


def _copy_layout_ds(ds: Mapping[str, Any]) -> Dict[str, Any]:
    """Copy the workflow pan-and-zoom metadata into the persisted layout shape."""

    scale = _coerce_float(ds.get("scale"), 1.0)
    offset = _coerce_vec2(ds.get("offset")) or [0.0, 0.0]
    return {"scale": scale, "offset": offset}


def _extract_layout_flags(workflow_node: Mapping[str, Any]) -> Dict[str, Any]:
    """Extract supported layout flags from a workflow node payload."""

    flags = workflow_node.get("flags")
    if not isinstance(flags, Mapping):
        return {}
    result: Dict[str, Any] = {}
    collapsed = flags.get("collapsed")
    if isinstance(collapsed, bool):
        result["collapsed"] = collapsed
    return result


def _extract_execution_mode(
    workflow_node: Optional[Mapping[str, Any]],
) -> Optional[int]:
    """Extract non-default LiteGraph execution mode from a workflow node."""

    if not isinstance(workflow_node, Mapping):
        return None
    value = workflow_node.get("mode")
    if isinstance(value, bool) or not isinstance(value, int):
        return None
    if value <= 0:
        return None
    return value


def _extract_layout_style(workflow_node: Mapping[str, Any]) -> Dict[str, Any]:
    """Extract stylistic overrides (color/bgcolor/shape) from the workflow node."""
    result: Dict[str, Any] = {}
    for key in _LAYOUT_STYLE_KEYS_STR:
        value = workflow_node.get(key)
        if isinstance(value, str):
            trimmed = value.strip()
            if trimmed:
                result[key] = trimmed
        elif isinstance(value, (int, float)):
            result[key] = value
    shape = workflow_node.get("shape")
    if isinstance(shape, (int, float)):
        result.setdefault("shape", shape)
    elif isinstance(shape, str):
        trimmed = shape.strip()
        if trimmed:
            result.setdefault("shape", trimmed)
    return result


def _coerce_layout_ds(value: Any) -> Dict[str, Any]:
    """Coerce workflow pan-and-zoom metadata into the persisted layout shape."""

    if isinstance(value, Mapping):
        return _copy_layout_ds(value)
    return {"scale": 1.0, "offset": [0.0, 0.0]}


def _coerce_vec2(value: Any) -> Optional[List[float]]:
    """Coerce a two-element sequence into numeric coordinates."""

    if isinstance(value, (list, tuple)) and len(value) == 2:
        try:
            return [float(value[0]), float(value[1])]
        except (TypeError, ValueError):
            return None
    return None


def _coerce_vec4(value: Any) -> Optional[List[float]]:
    """Coerce a four-element sequence into a bounding box list."""

    if isinstance(value, (list, tuple)) and len(value) == 4:
        try:
            return [float(value[0]), float(value[1]), float(value[2]), float(value[3])]
        except (TypeError, ValueError):
            return None
    return None


def _coerce_float(value: Any, default: float) -> float:
    """Coerce numeric-like layout values while keeping invalid input harmless."""

    if value is None or isinstance(value, bool):
        return default
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        stripped = value.strip()
        if not stripped:
            return default
        try:
            return float(stripped)
        except ValueError:
            return default
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _coerce_int_value(value: Any) -> Optional[int]:
    """Coerce workflow version values into integers when they are parseable."""

    if value is None or isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return int(value)
    if isinstance(value, str):
        stripped = value.strip()
        if not stripped:
            return None
        try:
            return int(stripped)
        except ValueError:
            try:
                return int(float(stripped))
            except (OverflowError, ValueError):
                return None
    try:
        return int(value)
    except (OverflowError, TypeError, ValueError):
        return None


def _round_value(value: float) -> float:
    """Round persisted layout values to a stable precision."""

    return round(float(value), 6)


def _downstream_edges(
    marker: CubeMarker, graph: Graph, subgraph: Sequence[str]
) -> List[Edge]:
    """Collect edges from a marker into executable nodes within the cube."""

    subgraph_set = set(subgraph)
    edges = [
        edge for edge in graph.edges_from(marker.node_id) if edge.target in subgraph_set
    ]
    edges.sort(
        key=lambda edge: (_natural_key(edge.target), str(edge.target_port or ""))
    )
    return edges


def _upstream_edges(
    marker: CubeMarker, graph: Graph, subgraph: Sequence[str]
) -> List[Edge]:
    """Collect edges from executable nodes into an output marker."""

    subgraph_set = set(subgraph)
    edges = [
        edge for edge in graph.edges_to(marker.node_id) if edge.source in subgraph_set
    ]
    edges.sort(key=lambda edge: (_natural_key(edge.source), int(edge.source_slot or 0)))
    return edges


def _downstream_connections(
    edges: Sequence[Edge], symbols: Mapping[str, str]
) -> List[List[Any]]:
    """Convert downstream edges into serialized binding targets."""

    return [[symbols[edge.target], edge.target_port] for edge in edges]


def _resolve_input_binding_type(
    edges: Sequence[Edge], graph: Graph, definitions: Mapping[str, Any]
) -> str:
    """Resolve the normalized type label for one serialized input binding."""

    if not edges:
        return "value"
    edge = edges[0]
    node = graph.nodes.get(edge.target)
    if not node:
        return "value"
    resolved = resolve_input_type(definitions, node.class_type, edge.target_port)
    return _sanitize_identifier(resolved or "value")


def _resolve_output_binding_type(
    edge: Edge, graph: Graph, definitions: Mapping[str, Any]
) -> str:
    """Resolve the normalized type label for one serialized output binding."""

    node = graph.nodes.get(edge.source)
    if not node:
        return "value"
    resolved = resolve_output_type_by_slot(
        definitions, node.class_type, edge.source_slot
    )
    return _sanitize_identifier(resolved or "value")


def _make_binding_key(
    direction: str, binding_type: str, counters: Dict[Tuple[str, str], int]
) -> str:
    """Build a stable binding alias that stays unique within one cube."""

    base = _sanitize_identifier(binding_type) or "value"
    key = (direction, base)
    count = counters.get(key, 0) + 1
    counters[key] = count
    suffix = "" if count == 1 else str(count)
    return f"{direction}.{base}{suffix}"


def _remap_value(
    value: Any,
    symbols: Mapping[str, str],
    alias_lookup: Mapping[str, str],
) -> Any:
    """Rewrite node references into serialized symbols and binding aliases."""

    if isinstance(value, list):
        if len(value) == 2 and _looks_like_link(value):
            source = str(value[0])
            slot = value[1]
            if source in symbols:
                return [symbols[source], slot]
            if source in alias_lookup:
                return [BINDING_SENTINEL, alias_lookup[source]]
            return value
        return [_remap_value(item, symbols, alias_lookup) for item in value]
    if isinstance(value, dict):
        return {k: _remap_value(v, symbols, alias_lookup) for k, v in value.items()}
    return value


def _symbol_base(node: GraphNode) -> str:
    """Choose the human-readable base used for generated node symbols."""

    title = node.meta.get("title")
    if isinstance(title, str) and title.strip():
        return _sanitize_identifier(title)
    class_type = node.class_type.split(".")[-1]
    return _sanitize_identifier(class_type)


def _sanitize_identifier(text: str) -> str:
    """Normalize arbitrary text into a stable symbol-safe identifier."""

    cleaned = re.sub(r"[^0-9a-zA-Z_]+", "_", text).strip("_")
    return cleaned.lower() or "node"


def _dedupe(base: str, used: MutableMapping[str, int]) -> str:
    """Append a numeric suffix when a generated identifier repeats."""

    count = used.get(base, 0)
    if count == 0:
        used[base] = 1
        return base
    count += 1
    used[base] = count
    return f"{base}_{count}"


def _natural_key(value: str) -> Tuple[int, str]:
    """Sort numeric ids numerically before falling back to lexical sorting."""

    try:
        return (0, f"{int(value):010d}")
    except ValueError:
        return (1, value)


def _looks_like_link(value: Sequence[Any]) -> bool:
    """Return whether the value matches ComfyUI's serialized link shape."""

    if len(value) != 2:
        return False
    node_id, slot = value
    return isinstance(node_id, (str, int)) and isinstance(slot, int)
