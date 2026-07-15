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
"""Serialize reusable Comfy workflow layout metadata for cube exports."""

from __future__ import annotations

import math
from copy import deepcopy
from dataclasses import dataclass
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple

from ..cube_model import build_cube_definition_key
from .graph import CubeData, CubeMarker, Graph, GraphNode
from .ordering import natural_node_key

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


@dataclass
class WorkflowLayoutIndex:
    """Capture workflow layout data needed during cube serialization."""

    nodes: Dict[str, Mapping[str, Any]]
    ds: Dict[str, Any]
    groups: List[Dict[str, Any]]
    version: Optional[int]


def build_workflow_index(
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
    version = coerce_int_value(workflow.get("version"))
    return WorkflowLayoutIndex(nodes=nodes, ds=ds, groups=groups, version=version)




def build_layout_payload(
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

    for node_id in sorted(cube.subgraph_nodes, key=natural_node_key):
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


def resolve_node_label(
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
        "x": coerce_float(padding.get("x"), _DEFAULT_CHROME_PADDING_X),
        "y": coerce_float(padding.get("y"), _DEFAULT_CHROME_PADDING_Y),
        "top_extra": coerce_float(
            padding.get("top_extra"), _DEFAULT_CHROME_PADDING_TOP_EXTRA
        ),
    }


def _resolve_chrome_header(bounds: Any) -> Dict[str, float]:
    """Resolve reusable chrome header metadata from saved bounds."""

    header = bounds.get("header") if isinstance(bounds, Mapping) else None
    if not isinstance(header, Mapping):
        header = {}
    return {
        "height": coerce_float(header.get("height"), _DEFAULT_CHROME_HEADER_HEIGHT)
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

    scale = coerce_float(ds.get("scale"), 1.0)
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


def extract_execution_mode(
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


def coerce_float(value: Any, default: float) -> float:
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


def coerce_int_value(value: Any) -> Optional[int]:
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

