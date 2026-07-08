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
"""Loader utilities for SugarCubes cube imports."""

from __future__ import annotations

import importlib
import json
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, Iterator, List, Mapping, Optional, Sequence, Set, Tuple

try:
    from ..cube_model import (
        CubeDocument,
        CubeIdentityError,
        CubeSchemaError,
        compute_surface_signature,
        derive_route_from_cube_id,
        derive_target_model_from_cube_id,
        looks_like_legacy_cube_payload,
        validate_cube_route_identity,
    )
    from ..cube_model.merge import materialize_nodes
    from ..instrumentation import log_event
except ImportError:
    from cube_model import (
        CubeDocument,
        CubeIdentityError,
        CubeSchemaError,
        compute_surface_signature,
        derive_route_from_cube_id,
        derive_target_model_from_cube_id,
        looks_like_legacy_cube_payload,
        validate_cube_route_identity,
    )
    from cube_model.merge import materialize_nodes
    from instrumentation import log_event

try:
    from ..nodes import NODE_CLASS_MAPPINGS as SUGAR_NODE_MAPPINGS
except ImportError:
    from nodes import NODE_CLASS_MAPPINGS as SUGAR_NODE_MAPPINGS

BINDING_SENTINEL = "@binding"
_DEFAULT_NODE_SIZE = [180.0, 60.0]
_DEFAULT_MARKER_SIZE = [140.0, 46.0]
_GRID_COLUMNS = 3
_GRID_X_STEP = 320.0
_GRID_Y_STEP = 260.0
_UUID_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)
_COMFY_NODES_RESOLVED = False
_COMFY_NODES_MODULE: Any = None


class CubeImportError(RuntimeError):
    """Raised when a cube cannot be imported due to schema issues."""

    def __init__(self, message: str, *, details: Optional[Dict[str, Any]] = None):
        """Capture a user-facing import error plus optional structured details."""

        super().__init__(message)
        self.message = message
        self.details = details or {}


@dataclass
class CubeLayoutEntry:
    """Geometry metadata for a node or marker."""

    id: Optional[str]
    class_type: Optional[str]
    title: Optional[str]
    pos: Optional[Tuple[float, float]]
    size: Optional[Tuple[float, float]]
    extra: Dict[str, Any] = field(default_factory=dict)


@dataclass
class CubeLayout:
    """Overall layout context captured by the exporter."""

    origin: Tuple[float, float]
    ds: Dict[str, Any]
    nodes: Dict[str, CubeLayoutEntry] = field(default_factory=dict)
    markers: Dict[str, CubeLayoutEntry] = field(default_factory=dict)
    groups: List[Dict[str, Any]] = field(default_factory=list)


@dataclass
class CubeNode:
    """Parsed node entry from a cube."""

    symbol: str
    class_type: str
    inputs: Dict[str, Any]
    data: Dict[str, Any]
    layout: Optional[CubeLayoutEntry] = None


@dataclass
class CubeMarker:
    """Parsed marker metadata from a cube."""

    alias: str
    kind: str
    class_type: str
    widget_values: Dict[str, Any] = field(default_factory=dict)
    layout: Optional[CubeLayoutEntry] = None


@dataclass
class CubeInputSpec:
    """Description of an input binding defined by the cube."""

    alias: str
    kind: str
    targets: List[Tuple[str, Any]]


@dataclass
class CubeOutputSpec:
    """Description of an output binding defined by the cube."""

    alias: str
    source_symbol: str
    source_slot: Optional[Any] = None


@dataclass
class LoadedCube:
    """Normalized in-memory representation of a cube file."""

    cube_id: str
    version: str
    nodes: Dict[str, CubeNode]
    markers: Dict[str, CubeMarker]
    inputs: Dict[str, CubeInputSpec]
    outputs: Dict[str, CubeOutputSpec]
    layout: Optional[CubeLayout]
    warnings: List[str]
    description: str = ""
    metadata: Dict[str, Any] = field(default_factory=dict)
    definitions: Dict[str, Any] = field(default_factory=dict)
    subgraphs: List[Dict[str, Any]] = field(default_factory=list)
    surface: Dict[str, Any] = field(default_factory=dict)
    flavors: Dict[str, Any] = field(default_factory=dict)
    surface_signature: str = ""


@dataclass
class PreparedImport:
    """Payload ready to send back to the frontend importer."""

    nodes: List[Dict[str, Any]]
    markers: List[Dict[str, Any]]
    connections: List[Dict[str, Any]]
    layout: Optional[Dict[str, Any]]
    warnings: List[str]
    cube: Dict[str, Any] = field(default_factory=dict)
    subgraphs: List[Dict[str, Any]] = field(default_factory=list)


_MARKER_KIND_TO_CLASS = {
    "input": "SugarCubes.CubeInput",
    "output": "SugarCubes.CubeOutput",
}


def load_cube(path: Path | str) -> LoadedCube:
    """Load and validate a cube JSON file from disk."""

    cube_path = Path(path)
    if not cube_path.exists():
        raise CubeImportError(f"Cube file not found: {cube_path}")

    try:
        with cube_path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError as exc:
        raise CubeImportError(
            "Cube file is not valid JSON",
            details={"path": str(cube_path)},
        ) from exc

    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube root must be a JSON object")

    if looks_like_legacy_cube_payload(payload):
        cube_id = _coerce_str(payload.get("cube_id")) or ""
        raise CubeImportError(
            "Legacy cube format is unsupported. Run scripts/migrate_legacy_cubes.py.",
            details={
                "legacy": True,
                "cube_id": cube_id,
                "path": str(cube_path),
            },
        )

    try:
        document = CubeDocument.from_dict(payload)
    except CubeSchemaError as exc:
        raise CubeImportError(str(exc), details={"path": str(cube_path)}) from exc

    warnings: List[str] = []
    description = document.description
    cube_id = document.cube_id
    version = document.version
    metadata = dict(document.metadata)
    definitions = dict(document.implementation.definitions)
    subgraphs = _parse_subgraphs(document.implementation.subgraphs, warnings)

    subgraph_ids = {
        subgraph_id
        for subgraph_id in (_coerce_str(entry.get("id")) for entry in subgraphs)
        if subgraph_id
    }
    nodes = _parse_nodes(
        materialize_nodes(document),
        definitions,
        warnings,
        subgraph_ids=subgraph_ids,
    )
    inputs = _parse_inputs(document.implementation.inputs, warnings)
    outputs = _parse_outputs(document.implementation.outputs, warnings)
    default_alias = _resolve_default_alias(document.to_dict(), cube_path.stem, cube_id)
    markers = _build_markers(
        inputs,
        outputs,
        cube_id=cube_id,
        default_alias=default_alias,
    )
    layout = _parse_layout(document.implementation.layout, nodes, markers, warnings)

    if layout:
        for symbol, node in nodes.items():
            node.layout = layout.nodes.get(symbol)
        for alias, marker in markers.items():
            marker.layout = layout.markers.get(alias)

    layout_summary = {
        "present": bool(layout),
        "node_entries": len(layout.nodes) if layout else 0,
        "marker_entries": len(layout.markers) if layout else 0,
        "groups": len(layout.groups) if layout else 0,
    }
    attached_nodes = sum(1 for node in nodes.values() if node.layout is not None)
    attached_markers = sum(
        1 for marker in markers.values() if marker.layout is not None
    )

    loaded_cube = LoadedCube(
        cube_id=cube_id,
        version=version,
        nodes=nodes,
        markers=markers,
        inputs=inputs,
        outputs=outputs,
        layout=layout,
        warnings=warnings,
        description=description,
        metadata=dict(metadata),
        definitions=dict(definitions),
        subgraphs=subgraphs,
        surface=document.surface.to_dict(),
        flavors=document.flavors.to_dict(),
        surface_signature=compute_surface_signature(document.surface),
    )

    log_event(
        "importer.phase3",
        "load_cube",
        {
            "path": str(cube_path),
            "cube_id": cube_id,
            "node_count": len(nodes),
            "marker_count": len(markers),
            "input_count": len(inputs),
            "output_count": len(outputs),
            "layout": layout_summary,
            "layout_attached": {"nodes": attached_nodes, "markers": attached_markers},
            "warnings": warnings,
        },
    )
    return loaded_cube


def prepare_import(
    loaded: LoadedCube,
    *,
    drop_origin: Sequence[float] | Tuple[float, float] = (0.0, 0.0),
) -> PreparedImport:
    """Convert a `LoadedCube` into an importer response payload."""

    dx, dy = _coerce_vec2_tuple(drop_origin)
    warnings = list(loaded.warnings)

    if loaded.layout:
        base_origin = (
            _round_value(loaded.layout.origin[0] + dx),
            _round_value(loaded.layout.origin[1] + dy),
        )
        layout_info: Optional[Dict[str, Any]] = {
            "origin": [base_origin[0], base_origin[1]],
            "original_origin": [
                _round_value(loaded.layout.origin[0]),
                _round_value(loaded.layout.origin[1]),
            ],
            "ds": dict(loaded.layout.ds),
            "groups": list(loaded.layout.groups),
        }
    else:
        warnings.append("Cube layout metadata missing; falling back to grid placement")
        base_origin = (_round_value(dx), _round_value(dy))
        layout_info = {
            "origin": [base_origin[0], base_origin[1]],
            "ds": {"scale": 1.0, "offset": [0.0, 0.0]},
            "groups": [],
        }

    node_entries: List[Dict[str, Any]] = []
    for idx, symbol in enumerate(sorted(loaded.nodes)):
        node = loaded.nodes[symbol]
        if loaded.layout:
            node_entries.append(_build_node_entry(node, base_origin, idx))
        else:
            node_entries.append(
                _build_node_entry_without_layout(node, base_origin, idx)
            )

    marker_entries: List[Dict[str, Any]] = []
    offset = len(node_entries)
    for idx, alias in enumerate(sorted(loaded.markers)):
        marker = loaded.markers[alias]
        if loaded.layout:
            marker_entries.append(_build_marker_entry(marker, base_origin, idx))
        else:
            marker_entries.append(
                _build_marker_entry_without_layout(marker, base_origin, offset + idx)
            )

    connections = _collect_node_connections(loaded.nodes)
    connections.extend(_collect_output_connections(loaded.outputs))

    default_alias = (
        _coerce_str(loaded.metadata.get("default_alias")) or Path(loaded.cube_id).stem
    )
    cube_payload = {
        "description": loaded.description,
        "cube_id": loaded.cube_id,
        "version": loaded.version,
        "metadata": loaded.metadata,
        "default_alias": default_alias,
        "target_model": _coerce_str(loaded.metadata.get("target_model"))
        or _derive_target_model(loaded.cube_id),
        "definitions": loaded.definitions,
        "surface": loaded.surface,
        "flavors": loaded.flavors,
        "surface_signature": loaded.surface_signature,
    }

    prepared = PreparedImport(
        nodes=node_entries,
        markers=marker_entries,
        connections=connections,
        layout=layout_info,
        warnings=warnings,
        cube=cube_payload,
        subgraphs=list(loaded.subgraphs),
    )

    log_event(
        "importer.phase3",
        "prepare_import",
        {
            "node_count": len(node_entries),
            "marker_count": len(marker_entries),
            "connection_count": len(connections),
            "layout_present": bool(loaded.layout),
            "base_origin": [base_origin[0], base_origin[1]],
            "drop_origin": [dx, dy],
            "warnings": warnings,
        },
    )
    return prepared


def _parse_nodes(
    payload: Any,
    definitions: Mapping[str, Any],
    warnings: List[str],
    *,
    subgraph_ids: Optional[Set[str]] = None,
) -> Dict[str, CubeNode]:
    """Parse serialized node payloads and warn on missing class definitions."""

    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube 'nodes' must be an object")

    nodes: Dict[str, CubeNode] = {}
    missing_definitions: List[str] = []

    for raw_symbol, data in payload.items():
        symbol = _coerce_symbol(raw_symbol, "node symbol")
        if not isinstance(data, Mapping):
            raise CubeImportError(f"Node '{symbol}' must be an object")

        class_type = _coerce_str(data.get("class_type"))
        if not class_type:
            raise CubeImportError(f"Node '{symbol}' is missing a valid 'class_type'")

        inputs = data.get("inputs")
        if inputs is None:
            inputs = {}
        if not isinstance(inputs, Mapping):
            raise CubeImportError(
                f"Node '{symbol}' inputs must be an object if provided"
            )

        extras = {k: v for k, v in data.items() if k not in {"class_type", "inputs"}}
        mode = _coerce_execution_mode(extras.get("mode"))
        if "mode" in extras and mode is None:
            warnings.append(f"Node '{symbol}' mode is invalid and was ignored")
            extras.pop("mode", None)
        elif mode is not None:
            extras["mode"] = mode
        node = CubeNode(
            symbol=symbol,
            class_type=class_type,
            inputs=dict(inputs),
            data=dict(extras),
        )
        nodes[symbol] = node

        if not _has_definition(class_type, definitions, subgraph_ids=subgraph_ids):
            missing_definitions.append(class_type)

    if missing_definitions:
        unique = sorted(set(missing_definitions))
        warnings.append("Undefined node classes referenced: " + ", ".join(unique))

    return nodes


def _parse_inputs(payload: Any, warnings: List[str]) -> Dict[str, CubeInputSpec]:
    """Parse serialized input bindings into normalized input specs."""

    if payload is None:
        return {}
    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube 'inputs' must be an object")

    inputs: Dict[str, CubeInputSpec] = {}

    for raw_alias, data in payload.items():
        alias = _coerce_symbol(raw_alias, "input alias")
        if not isinstance(data, Mapping):
            raise CubeImportError(f"Input '{alias}' must be an object")

        kind = data.get("kind")
        if kind != "input":
            raise CubeImportError(f"Input '{alias}' has unsupported kind '{kind}'")

        targets_raw = data.get("targets")
        if targets_raw is None:
            targets_list: List[Tuple[str, Any]] = []
        elif isinstance(targets_raw, (list, tuple)):
            targets_list = []
            for idx, target in enumerate(targets_raw):
                parsed = _parse_target(target)
                if parsed is None:
                    warnings.append(
                        f"Input '{alias}' target #{idx + 1} is invalid: {target!r}"
                    )
                    continue
                targets_list.append(parsed)
        else:
            raise CubeImportError(f"Input '{alias}' targets must be an array")

        inputs[alias] = CubeInputSpec(alias=alias, kind=kind, targets=targets_list)

    return inputs


def _parse_outputs(payload: Any, warnings: List[str]) -> Dict[str, CubeOutputSpec]:
    """Parse serialized cube outputs into normalized output specs."""

    if payload is None:
        return {}
    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube 'outputs' must be an object")

    outputs: Dict[str, CubeOutputSpec] = {}

    for raw_alias, value in payload.items():
        alias = _coerce_symbol(raw_alias, "output alias")
        source_symbol: Optional[str]
        source_slot: Optional[Any]

        if isinstance(value, str) and value.strip():
            source_symbol = value.strip()
            source_slot = 0
        elif isinstance(value, (list, tuple)) and len(value) == 2:
            source_symbol = _coerce_str(value[0]) or str(value[0])
            source_slot = value[1]
        elif isinstance(value, Mapping):
            source_symbol = _coerce_str(value.get("symbol")) or _coerce_str(
                value.get("node")
            )
            source_slot = value.get("slot")
        else:
            raise CubeImportError(f"Output '{alias}' must specify a node reference")

        if not source_symbol:
            raise CubeImportError(f"Output '{alias}' missing target symbol")

        outputs[alias] = CubeOutputSpec(
            alias=alias, source_symbol=source_symbol, source_slot=source_slot
        )

    return outputs


def _build_markers(
    inputs: Mapping[str, CubeInputSpec],
    outputs: Mapping[str, CubeOutputSpec],
    *,
    cube_id: str,
    default_alias: str,
) -> Dict[str, CubeMarker]:
    """Build importer marker payloads from normalized input and output specs."""

    markers: Dict[str, CubeMarker] = {}

    for alias, spec in inputs.items():
        widget_values = {
            "cube_id": cube_id,
            "default_alias": default_alias,
            "instance_alias": default_alias,
        }
        markers[alias] = CubeMarker(
            alias=alias,
            kind=spec.kind,
            class_type=_MARKER_KIND_TO_CLASS["input"],
            widget_values=widget_values,
        )

    for alias, spec in outputs.items():
        if alias in markers:
            continue
        class_type = _MARKER_KIND_TO_CLASS.get("output", "SugarCubes.CubeOutput")
        widget_values = {
            "cube_id": cube_id,
            "default_alias": default_alias,
            "instance_alias": default_alias,
        }
        markers[alias] = CubeMarker(
            alias=alias,
            kind="output",
            class_type=class_type,
            widget_values=widget_values,
        )

    return markers


def _parse_layout(
    payload: Any,
    nodes: Mapping[str, CubeNode],
    markers: Mapping[str, CubeMarker],
    warnings: List[str],
) -> Optional[CubeLayout]:
    """Parse persisted layout metadata and attach it to nodes and markers."""

    if payload is None:
        warnings.append("Cube layout section missing")
        return None
    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube 'layout' must be an object")

    origin = _coerce_vec2(payload.get("origin"))
    if origin is None:
        warnings.append("Layout origin missing or invalid; defaulting to [0, 0]")
        origin = [0.0, 0.0]

    ds = _coerce_layout_ds(payload.get("ds"))
    layout = CubeLayout(origin=(origin[0], origin[1]), ds=ds)

    node_entries = payload.get("nodes")
    if isinstance(node_entries, Mapping):
        for raw_symbol, raw_entry in node_entries.items():
            symbol = _coerce_symbol(raw_symbol, "layout node key")
            entry = _parse_layout_entry(raw_entry)
            layout.nodes[symbol] = entry
    elif node_entries is not None:
        warnings.append("layout.nodes ignored because it is not an object")

    marker_entries = payload.get("markers")
    if isinstance(marker_entries, Mapping):
        for raw_alias, raw_entry in marker_entries.items():
            alias = _coerce_symbol(raw_alias, "layout marker key")
            entry = _parse_layout_entry(raw_entry)
            layout.markers[alias] = entry
    elif marker_entries is not None:
        warnings.append("layout.markers ignored because it is not an object")

    groups_raw = payload.get("groups")
    if isinstance(groups_raw, list):
        layout.groups = [
            dict(group) for group in groups_raw if isinstance(group, Mapping)
        ]

    for symbol in nodes:
        if symbol not in layout.nodes:
            warnings.append(f"Layout missing node entry for '{symbol}'")

    for alias in markers:
        if alias not in layout.markers:
            warnings.append(f"Layout missing marker entry for '{alias}'")

    return layout


def _resolve_default_alias(
    payload: Mapping[str, Any],
    fallback: str,
    cube_id: str,
) -> str:
    """Resolve the route-based default alias for imported markers."""

    metadata = payload.get("metadata")
    if isinstance(metadata, Mapping):
        name = _coerce_str(metadata.get("default_alias"))
        if name:
            try:
                validate_cube_route_identity(cube_id, name)
            except CubeIdentityError as exc:
                raise CubeImportError(str(exc)) from exc
            return name
    try:
        return derive_route_from_cube_id(cube_id)
    except CubeIdentityError:
        pass
    return fallback


def _derive_target_model(cube_id: str) -> str:
    """Return the target model implied by a cube id."""

    try:
        return derive_target_model_from_cube_id(cube_id)
    except CubeIdentityError:
        return ""


def _parse_subgraphs(payload: Any, warnings: List[str]) -> List[Dict[str, Any]]:
    """Parse persisted workflow subgraphs used by UUID wrapper nodes."""

    _ = warnings
    if payload is None:
        return []
    if not isinstance(payload, list):
        raise CubeImportError("Cube 'subgraphs' must be an array")
    parsed: List[Dict[str, Any]] = []
    for idx, entry in enumerate(payload):
        if not isinstance(entry, Mapping):
            raise CubeImportError(f"Subgraph entry #{idx + 1} must be an object")
        sub_id = entry.get("id")
        if not isinstance(sub_id, str) or not sub_id.strip():
            raise CubeImportError(f"Subgraph entry #{idx + 1} is missing id")
        nodes = entry.get("nodes")
        if not isinstance(nodes, list) or not nodes:
            raise CubeImportError(
                f"Subgraph '{sub_id.strip()}' must include a non-empty nodes array"
            )
        if not any(
            isinstance(node, Mapping)
            and isinstance(node.get("type") or node.get("class_type"), str)
            and str(node.get("type") or node.get("class_type")).strip()
            for node in nodes
        ):
            raise CubeImportError(
                f"Subgraph '{sub_id.strip()}' does not contain executable node entries"
            )
        parsed.append(dict(entry))
    _validate_subgraph_references(parsed)
    return parsed


def _validate_subgraph_references(subgraphs: Sequence[Mapping[str, Any]]) -> None:
    """Reject persisted subgraphs that reference missing nested definitions."""

    available_ids = {
        subgraph_id
        for subgraph_id in (_coerce_str(entry.get("id")) for entry in subgraphs)
        if subgraph_id
    }
    for entry in subgraphs:
        parent_id = _coerce_str(entry.get("id")) or "<unknown>"
        missing_ids = sorted(
            _collect_subgraph_wrapper_references(entry) - available_ids
        )
        if missing_ids:
            raise CubeImportError(
                f"Subgraph '{parent_id}' references missing nested subgraph "
                f"definition '{missing_ids[0]}'"
            )


def _collect_subgraph_wrapper_references(entry: Mapping[str, Any]) -> Set[str]:
    """Return nested UUID wrapper ids referenced by one persisted subgraph."""

    nodes = entry.get("nodes")
    if not isinstance(nodes, Sequence) or isinstance(nodes, (str, bytes)):
        return set()

    references: Set[str] = set()
    for node in nodes:
        if not isinstance(node, Mapping):
            continue
        class_type = node.get("type")
        if not isinstance(class_type, str):
            class_type = node.get("class_type")
        normalized = class_type.strip() if isinstance(class_type, str) else ""
        if normalized and _UUID_RE.match(normalized):
            references.add(normalized)
    return references


def _build_node_entry(
    node: CubeNode, base_origin: Tuple[float, float], order: int
) -> Dict[str, Any]:
    """Build one importer node payload while preserving layout metadata."""

    layout = node.layout
    if layout and layout.pos is not None:
        pos = [
            _round_value(base_origin[0] + layout.pos[0]),
            _round_value(base_origin[1] + layout.pos[1]),
        ]
    else:
        pos = _grid_position(order, base_origin)
    size = list(layout.size) if layout and layout.size else list(_DEFAULT_NODE_SIZE)

    entry: Dict[str, Any] = {
        "symbol": node.symbol,
        "class_type": node.class_type,
        "inputs": node.inputs,
        "extras": dict(node.data),
        "layout": {
            "id": layout.id if layout else None,
            "title": layout.title if layout else None,
            "class_type": layout.class_type if layout else node.class_type,
            "pos": pos,
            "size": size,
        },
    }
    mode = _coerce_execution_mode(node.data.get("mode"))
    if mode is not None:
        entry["mode"] = mode
    if layout and layout.extra:
        extra = dict(layout.extra)
        entry["layout"]["extra"] = extra
        flags = extra.get("flags")
        if isinstance(flags, dict):
            entry["layout"]["flags"] = dict(flags)
        style = extra.get("style")
        if isinstance(style, dict):
            entry["layout"]["style"] = dict(style)
    return entry


def _build_node_entry_without_layout(
    node: CubeNode, base_origin: Tuple[float, float], order: int
) -> Dict[str, Any]:
    """Build one importer node payload using grid fallback placement."""

    pos = _grid_position(order, base_origin)
    entry: Dict[str, Any] = {
        "symbol": node.symbol,
        "class_type": node.class_type,
        "inputs": node.inputs,
        "extras": dict(node.data),
        "layout": {
            "id": None,
            "title": None,
            "class_type": node.class_type,
            "pos": pos,
            "size": list(_DEFAULT_NODE_SIZE),
        },
    }
    mode = _coerce_execution_mode(node.data.get("mode"))
    if mode is not None:
        entry["mode"] = mode
    return entry


def _build_marker_entry(
    marker: CubeMarker, base_origin: Tuple[float, float], order: int
) -> Dict[str, Any]:
    """Build one importer marker payload while preserving layout metadata."""

    layout = marker.layout
    if layout and layout.pos is not None:
        pos = [
            _round_value(base_origin[0] + layout.pos[0]),
            _round_value(base_origin[1] + layout.pos[1]),
        ]
    else:
        pos = _grid_position(order, base_origin)
    size = list(layout.size) if layout and layout.size else list(_DEFAULT_MARKER_SIZE)

    entry: Dict[str, Any] = {
        "alias": marker.alias,
        "kind": marker.kind,
        "class_type": marker.class_type,
        "widget_values": dict(marker.widget_values),
        "layout": {
            "id": layout.id if layout else None,
            "title": (
                layout.title
                if layout
                else _derive_marker_title(marker.alias, marker.kind)
            ),
            "class_type": layout.class_type if layout else marker.class_type,
            "pos": pos,
            "size": size,
        },
    }
    if layout and layout.extra:
        extra = dict(layout.extra)
        entry["layout"]["extra"] = extra
        flags = extra.get("flags")
        if isinstance(flags, dict):
            entry["layout"]["flags"] = dict(flags)
        style = extra.get("style")
        if isinstance(style, dict):
            entry["layout"]["style"] = dict(style)
    return entry


def _build_marker_entry_without_layout(
    marker: CubeMarker, base_origin: Tuple[float, float], order: int
) -> Dict[str, Any]:
    """Build one importer marker payload using grid fallback placement."""

    pos = _grid_position(order, base_origin)
    entry: Dict[str, Any] = {
        "alias": marker.alias,
        "kind": marker.kind,
        "class_type": marker.class_type,
        "widget_values": dict(marker.widget_values),
        "layout": {
            "id": None,
            "title": _derive_marker_title(marker.alias, marker.kind),
            "class_type": marker.class_type,
            "pos": pos,
            "size": list(_DEFAULT_MARKER_SIZE),
        },
    }
    return entry


def _collect_node_connections(nodes: Mapping[str, CubeNode]) -> List[Dict[str, Any]]:
    """Collect node-to-node and binding-to-node connections for the frontend."""

    connections: List[Dict[str, Any]] = []
    for node in nodes.values():
        for input_name, value in node.inputs.items():
            for kind, symbol, slot in _iter_links(value):
                if kind == "binding":
                    connections.append(
                        {
                            "kind": "binding",
                            "from": {"symbol": symbol, "slot": 0},
                            "to": {"symbol": node.symbol, "input": input_name},
                        }
                    )
                else:
                    connections.append(
                        {
                            "kind": "link",
                            "from": {"symbol": symbol, "slot": slot},
                            "to": {"symbol": node.symbol, "input": input_name},
                        }
                    )
    return connections


def _collect_output_connections(
    outputs: Mapping[str, CubeOutputSpec],
) -> List[Dict[str, Any]]:
    """Collect output-marker connections for the frontend payload."""

    connections: List[Dict[str, Any]] = []
    for output in outputs.values():
        connections.append(
            {
                "kind": "output",
                "from": {
                    "symbol": output.source_symbol,
                    "slot": output.source_slot or 0,
                },
                "to": {"symbol": output.alias, "input": "value"},
            }
        )
    return connections


def _iter_links(value: Any) -> Iterator[Tuple[str, str, Any]]:
    """Yield binding and node links embedded inside serialized values."""

    if isinstance(value, list):
        if len(value) == 2 and isinstance(value[0], (str, int)):
            source = value[0]
            slot = value[1]
            if isinstance(source, str) and source == BINDING_SENTINEL:
                alias = _coerce_str(slot)
                if alias:
                    yield ("binding", alias, 0)
            else:
                yield ("node", str(source), slot)
        else:
            for item in value:
                yield from _iter_links(item)
    elif isinstance(value, tuple):
        for item in value:
            yield from _iter_links(item)
    elif isinstance(value, Mapping):
        for item in value.values():
            yield from _iter_links(item)


def _parse_layout_entry(raw: Any) -> CubeLayoutEntry:
    """Parse one layout entry while preserving unknown extra metadata."""

    if not isinstance(raw, Mapping):
        return CubeLayoutEntry(
            id=None, class_type=None, title=None, pos=None, size=None, extra={}
        )
    entry = dict(raw)
    entry_id = _coerce_str(entry.get("id"))
    class_type = _coerce_str(entry.get("class_type"))
    title = _coerce_str(entry.get("title"))
    pos = _coerce_vec2(entry.get("pos"))
    pos_tuple = (pos[0], pos[1]) if pos else None
    size = _coerce_vec2(entry.get("size"))
    size_tuple = (size[0], size[1]) if size else None

    known_keys = {"id", "class_type", "title", "pos", "size", "kind"}
    extra = {k: v for k, v in entry.items() if k not in known_keys}

    return CubeLayoutEntry(
        id=entry_id,
        class_type=class_type,
        title=title,
        pos=pos_tuple,
        size=size_tuple,
        extra=extra,
    )


def _grid_position(order: int, base_origin: Tuple[float, float]) -> List[float]:
    """Return the fallback grid position for one generated frontend entry."""

    col = order % _GRID_COLUMNS
    row = order // _GRID_COLUMNS
    x = base_origin[0] + col * _GRID_X_STEP
    y = base_origin[1] + row * _GRID_Y_STEP
    return [_round_value(x), _round_value(y)]


def _parse_target(value: Any) -> Optional[Tuple[str, Any]]:
    """Parse one input target reference when it matches the serialized shape."""

    if isinstance(value, (list, tuple)) and len(value) == 2:
        symbol = _coerce_str(value[0]) or str(value[0])
        slot = value[1]
        if symbol:
            return symbol, slot
    return None


def _has_definition(
    class_type: str,
    definitions: Mapping[str, Any],
    *,
    subgraph_ids: Optional[Set[str]] = None,
) -> bool:
    """Return whether the class type is defined by the cube, Comfy, or SugarCubes."""

    if class_type in definitions:
        return True
    if subgraph_ids and class_type in subgraph_ids:
        return True
    comfy_nodes = _load_comfy_nodes_module()
    if comfy_nodes and hasattr(comfy_nodes, "NODE_CLASS_MAPPINGS"):
        mapping = getattr(comfy_nodes, "NODE_CLASS_MAPPINGS")
        if class_type in mapping:
            return True
    if class_type in SUGAR_NODE_MAPPINGS:
        return True
    return False


def _load_comfy_nodes_module() -> Any:
    """Lazily load Comfy's node registry for import validation fallback."""

    global _COMFY_NODES_RESOLVED
    global _COMFY_NODES_MODULE

    if _COMFY_NODES_RESOLVED:
        return _COMFY_NODES_MODULE

    try:
        _COMFY_NODES_MODULE = importlib.import_module("nodes")
    except (ImportError, ModuleNotFoundError):
        _COMFY_NODES_MODULE = None
    _COMFY_NODES_RESOLVED = True
    return _COMFY_NODES_MODULE


def _coerce_symbol(value: Any, context: str) -> str:
    """Coerce a required symbol-like value or raise an import error."""

    symbol = _coerce_str(value)
    if not symbol:
        raise CubeImportError(f"Invalid {context}: {value!r}")
    return symbol


def _coerce_str(value: Any) -> Optional[str]:
    """Coerce primitives into trimmed strings when that preserves intent."""

    if isinstance(value, str):
        cleaned = value.strip()
        return cleaned if cleaned else None
    if isinstance(value, (int, float)):
        return str(value)
    return None


def _coerce_vec2(value: Any) -> Optional[List[float]]:
    """Coerce a two-element sequence into numeric layout coordinates."""

    if isinstance(value, (list, tuple)) and len(value) == 2:
        try:
            return [float(value[0]), float(value[1])]
        except (TypeError, ValueError):
            return None
    return None


def _coerce_vec2_tuple(
    value: Sequence[float] | Tuple[float, float],
) -> Tuple[float, float]:
    """Return a two-float tuple for importer placement calculations."""

    vec = _coerce_vec2(value)
    if vec is None:
        return (0.0, 0.0)
    return (vec[0], vec[1])


def _coerce_execution_mode(value: Any) -> Optional[int]:
    """Return a valid non-default LiteGraph execution mode."""

    if isinstance(value, bool) or not isinstance(value, int):
        return None
    if value <= 0:
        return None
    return value


def _coerce_layout_ds(value: Any) -> Dict[str, Any]:
    """Normalize the workflow pan-and-zoom metadata for importer reuse."""

    if isinstance(value, Mapping):
        scale = _coerce_float(value.get("scale"), 1.0)
        offset = _coerce_vec2(value.get("offset")) or [0.0, 0.0]
    else:
        scale = 1.0
        offset = [0.0, 0.0]
    return {"scale": scale, "offset": offset}


def _coerce_float(value: Any, default: float) -> float:
    """Coerce numeric-like values while keeping invalid layout values harmless."""

    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _round_value(value: float, places: int = 3) -> float:
    """Round layout coordinates to a stable precision for frontend payloads."""

    return round(float(value), places)


def _derive_marker_title(alias: str, kind: str) -> str:
    """Derive a readable marker title when layout metadata is absent."""

    suffix = alias.split(".")[-1].replace("_", " ").title()
    if kind:
        return f"{kind.title()} - {suffix}"
    return suffix
