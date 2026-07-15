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
import re
from dataclasses import dataclass
from typing import (
    Any,
    Callable,
    Collection,
    Dict,
    List,
    Mapping,
    MutableMapping,
    Optional,
    Sequence,
    Tuple,
)

from ..cube_model import (
    CubeIdentityError,
    derive_route_from_cube_id,
    migrate_legacy_payload,
    sanitize_authored_defaults_payload,
)
from ..cube_model.picker_fields import (
    find_input_field_spec,
    is_picker_field_spec,
)
from ..cube_model.widget_values import canonicalize_subgraph_widget_values
from ..instrumentation import log_event

from .definition_snapshot import (
    collect_definitions,
    collect_subgraph_node_types,
)
from .graph import CubeAnalysis, CubeData, CubeMarker, Graph, GraphNode, Edge
from .layout_serializer import (
    WorkflowLayoutIndex,
    build_layout_payload,
    build_workflow_index,
    extract_execution_mode,
    resolve_node_label,
)
from .versioning import resolve_input_type, resolve_output_type_by_slot
from .node_inputs import backfill_missing_widget_inputs
from .ordering import natural_node_key
from .subgraph_serializer import build_subgraph_index, collect_subgraphs
from .value_validation import (
    invalid_named_value_reason,
    validate_named_node_inputs,
    validate_subgraph_widget_values,
)

BindingResolver = Callable[[str], Mapping[str, Any]]
BINDING_SENTINEL = "@binding"

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


@dataclass
class ExportedCube:
    """Represent one serialized cube plus export-time warnings."""

    default_alias: str
    cube: Dict[str, Any]
    warnings: List[str]
    version_auto: bool = False


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
    workflow_ctx = build_workflow_index(workflow) if workflow is not None else None
    subgraph_defs = build_subgraph_index(workflow) if workflow else {}
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
    subgraphs, subgraph_warnings = collect_subgraphs(cube, graph, subgraph_defs)
    subgraph_class_types = collect_subgraph_node_types(subgraphs)
    definitions, validation_definitions, definition_warnings = collect_definitions(
        symbols, graph, resolver, extra_class_types=subgraph_class_types
    )
    subgraphs = canonicalize_subgraph_widget_values(subgraphs, validation_definitions)
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
    layout_payload, layout_warnings = build_layout_payload(
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
    for node_id in sorted(cube.subgraph_nodes, key=natural_node_key):
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

    for marker in sorted(cube.inputs, key=lambda entry: natural_node_key(entry.node_id)):
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

    for marker in sorted(cube.outputs, key=lambda entry: natural_node_key(entry.node_id)):
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
    for node_id in sorted(cube.subgraph_nodes, key=natural_node_key):
        node = graph.nodes[node_id]
        payload_inputs: Dict[str, Any] = {}
        for key, value in node.inputs.items():
            payload_inputs[key] = _remap_value(value, symbols, alias_lookup)
        definition = validation_definitions.get(node.class_type)
        backfill_missing_widget_inputs(
            payload_inputs,
            node,
            layout_ctx.nodes.get(node_id) if layout_ctx else None,
            definition if isinstance(definition, Mapping) else None,
        )
        if isinstance(definition, Mapping):
            validate_named_node_inputs(
                node_id=node.id,
                class_type=node.class_type,
                inputs=payload_inputs,
                definition=definition,
            )
        node_payload: Dict[str, Any] = {
            "class_type": node.class_type,
            "label": resolve_node_label(
                symbols[node_id],
                layout_ctx.nodes.get(node_id) if layout_ctx else None,
                node,
            ),
            "inputs": payload_inputs,
            "original_id": node_id,
        }
        workflow_node = layout_ctx.nodes.get(node_id) if layout_ctx else None
        mode = extract_execution_mode(workflow_node if workflow_node else node.data)
        if mode is not None:
            node_payload["mode"] = mode
        nodes[symbols[node_id]] = node_payload
    return nodes


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
    control_index: dict[str, Mapping[str, Any]] = {}
    for control in controls:
        if not isinstance(control, Mapping):
            continue
        control_id = control.get("control_id")
        if isinstance(control_id, str):
            control_index[control_id] = control
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
            if not isinstance(control_id, str):
                continue
            selected_control = control_index.get(control_id)
            if selected_control is None:
                continue
            _validate_authored_control_value(
                payload,
                selected_control,
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
    reason = invalid_named_value_reason(value, field_spec)
    if reason is None:
        return
    _raise_authored_value_error(
        payload=payload,
        control=control,
        field_spec=field_spec,
        flavor_id=flavor_id,
        value=value,
        reason=f"authored default does not match node definition ({reason})",
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
    if isinstance(version_value, str) and version_value.strip():
        version = version_value.strip()
        version_auto = False
    else:
        version = "1.0.0"
        version_auto = True
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




def _downstream_edges(
    marker: CubeMarker, graph: Graph, subgraph: Collection[str]
) -> List[Edge]:
    """Collect edges from a marker into executable nodes within the cube."""

    subgraph_set = set(subgraph)
    edges = [
        edge for edge in graph.edges_from(marker.node_id) if edge.target in subgraph_set
    ]
    edges.sort(
        key=lambda edge: (natural_node_key(edge.target), str(edge.target_port or ""))
    )
    return edges


def _upstream_edges(
    marker: CubeMarker, graph: Graph, subgraph: Collection[str]
) -> List[Edge]:
    """Collect edges from executable nodes into an output marker."""

    subgraph_set = set(subgraph)
    edges = [
        edge for edge in graph.edges_to(marker.node_id) if edge.source in subgraph_set
    ]
    edges.sort(key=lambda edge: (natural_node_key(edge.source), int(edge.source_slot or 0)))
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


def _looks_like_link(value: Sequence[Any]) -> bool:
    """Return whether the value matches ComfyUI's serialized link shape."""

    if len(value) != 2:
        return False
    node_id, slot = value
    return isinstance(node_id, (str, int)) and isinstance(slot, int)
