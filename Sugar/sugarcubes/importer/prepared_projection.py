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
"""Materialize normalized Cubes into frontend-ready import payloads."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple

from ..cube_model import CubeIdentityError, derive_target_model_from_cube_id
from ..instrumentation import log_event
from .boundary_projection import project_cube_boundaries
from .coercion import _coerce_str, _coerce_vec2_tuple, _round_value
from .connection_projection import (
    _collect_node_connections,
    _collect_output_connections,
)
from .entry_projection import (
    _build_marker_entry,
    _build_marker_entry_without_layout,
    _build_node_entry,
    _build_node_entry_without_layout,
)
from .models import CubeMarker, LoadedCube, PreparedImport


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
    boundaries = project_cube_boundaries(
        nodes=loaded.nodes,
        inputs=loaded.inputs,
        outputs=loaded.outputs,
        definitions=loaded.definitions,
        subgraphs=loaded.subgraphs,
        marker_titles=_collect_marker_titles(loaded.markers),
        warnings=warnings,
    )

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
        boundaries=boundaries,
    )

    log_event(
        "importer.phase3",
        "prepare_import",
        {
            "node_count": len(node_entries),
            "marker_count": len(marker_entries),
            "connection_count": len(connections),
            "boundary_inputs": len(boundaries["inputs"]),
            "boundary_outputs": len(boundaries["outputs"]),
            "layout_present": bool(loaded.layout),
            "base_origin": [base_origin[0], base_origin[1]],
            "drop_origin": [dx, dy],
            "warnings": warnings,
        },
    )
    return prepared


def _collect_marker_titles(markers: Mapping[str, CubeMarker]) -> Dict[str, str]:
    """Return optional legacy marker titles for native boundary presentation."""

    return {
        alias: marker.layout.title.strip()
        for alias, marker in markers.items()
        if marker.layout
        and isinstance(marker.layout.title, str)
        and marker.layout.title.strip()
    }


def _derive_target_model(cube_id: str) -> str:
    """Return the target model implied by a cube id."""

    try:
        return derive_target_model_from_cube_id(cube_id)
    except CubeIdentityError:
        return ""
