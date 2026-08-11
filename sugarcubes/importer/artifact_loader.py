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
"""Load and normalize Cube artifacts from disk."""

from __future__ import annotations

import json
from pathlib import Path
from typing import List, Mapping

from ..cube_model import (
    CubeDocument,
    CubeSchemaError,
    compute_surface_signature,
    looks_like_legacy_cube_payload,
    sanitize_authored_defaults_payload,
)
from ..cube_model.merge import materialize_nodes
from ..cube_model.widget_values import (
    WidgetSnapshotError,
    canonicalize_subgraph_widget_values,
)
from ..instrumentation import log_event
from .coercion import _coerce_str
from .layout_parser import _parse_layout
from .models import CubeImportError, LoadedCube
from .schema_parser import (
    _build_markers,
    _parse_inputs,
    _parse_nodes,
    _parse_outputs,
    _parse_subgraphs,
    _resolve_default_alias,
)


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

    portable_payload = document.to_dict()
    sanitize_authored_defaults_payload(
        portable_payload,
        definitions=document.implementation.definitions,
    )
    document = CubeDocument.from_dict(portable_payload)

    warnings: List[str] = []
    description = document.description
    cube_id = document.cube_id
    version = document.version
    metadata = dict(document.metadata)
    definitions = dict(document.implementation.definitions)
    subgraphs = _parse_subgraphs(document.implementation.subgraphs, warnings)
    try:
        subgraphs = canonicalize_subgraph_widget_values(subgraphs, definitions)
    except WidgetSnapshotError as exc:
        raise CubeImportError(f"Unsafe subgraph widget snapshot: {exc}") from exc

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
