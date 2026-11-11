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
"""Parse persisted Cube layout metadata independently of schema loading."""

from __future__ import annotations

from typing import Any, List, Mapping, Optional

from .coercion import _coerce_layout_ds, _coerce_str, _coerce_symbol, _coerce_vec2
from .models import CubeImportError, CubeLayout, CubeLayoutEntry, CubeMarker, CubeNode


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
