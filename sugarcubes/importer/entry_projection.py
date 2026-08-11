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
"""Project normalized Cube nodes and markers into frontend entries."""

from __future__ import annotations

from typing import Any, Dict, List, Tuple

from .coercion import (
    _coerce_execution_mode,
    _derive_marker_title,
    _round_value,
)
from .models import CubeMarker, CubeNode

_DEFAULT_NODE_SIZE = [180.0, 60.0]
_DEFAULT_MARKER_SIZE = [140.0, 46.0]
_GRID_COLUMNS = 3
_GRID_X_STEP = 320.0
_GRID_Y_STEP = 260.0


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


def _grid_position(order: int, base_origin: Tuple[float, float]) -> List[float]:
    """Return the fallback grid position for one generated frontend entry."""

    col = order % _GRID_COLUMNS
    row = order // _GRID_COLUMNS
    x = base_origin[0] + col * _GRID_X_STEP
    y = base_origin[1] + row * _GRID_Y_STEP
    return [_round_value(x), _round_value(y)]
