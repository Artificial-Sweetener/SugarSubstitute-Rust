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
"""Normalize validated in-memory Cube documents for native import projection."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path
from typing import List

from ..cube_model import (
    CubeDocument,
    compute_surface_signature,
    sanitize_authored_defaults_document,
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


def load_cube_document(
    document: CubeDocument, *, artifact_name: str | None = None
) -> LoadedCube:
    """Normalize one already-validated document without filesystem access."""

    document = sanitize_authored_defaults_document(document)
    return _normalize_document(
        document,
        materialize_nodes(document),
        artifact_name=artifact_name,
    )


def load_materialized_cube_document(
    document: CubeDocument, *, artifact_name: str | None = None
) -> LoadedCube:
    """Normalize a compiler-materialized document without replaying flavor defaults."""

    return _normalize_document(
        document,
        deepcopy(document.implementation.nodes),
        artifact_name=artifact_name,
    )


def _normalize_document(
    document: CubeDocument,
    runtime_nodes: dict[str, dict[str, object]],
    *,
    artifact_name: str | None,
) -> LoadedCube:
    """Project one validated document and its authoritative runtime node values."""

    warnings: List[str] = []
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
        runtime_nodes,
        definitions,
        warnings,
        subgraph_ids=subgraph_ids,
    )
    inputs = _parse_inputs(document.implementation.inputs, warnings)
    outputs = _parse_outputs(document.implementation.outputs, warnings)
    source_name = artifact_name or Path(document.cube_id).stem
    markers = _build_markers(
        inputs,
        outputs,
        cube_id=document.cube_id,
        default_alias=_resolve_default_alias(
            document.to_dict(), Path(source_name).stem, document.cube_id
        ),
    )
    layout = _parse_layout(document.implementation.layout, nodes, markers, warnings)
    if layout:
        for symbol, node in nodes.items():
            node.layout = layout.nodes.get(symbol)
        for alias, marker in markers.items():
            marker.layout = layout.markers.get(alias)

    loaded = LoadedCube(
        cube_id=document.cube_id,
        version=document.version,
        nodes=nodes,
        markers=markers,
        inputs=inputs,
        outputs=outputs,
        layout=layout,
        warnings=warnings,
        description=document.description,
        metadata=dict(document.metadata),
        definitions=definitions,
        subgraphs=subgraphs,
        surface=document.surface.to_dict(),
        flavors=document.flavors.to_dict(),
        surface_signature=compute_surface_signature(document.surface),
        document=document.to_dict(),
    )
    _log_loaded_document(loaded, source_name)
    return loaded


def _log_loaded_document(loaded: LoadedCube, source_name: str) -> None:
    """Emit one structured summary after complete document normalization."""

    layout = loaded.layout
    log_event(
        "importer.phase3",
        "load_cube",
        {
            "path": source_name,
            "cube_id": loaded.cube_id,
            "node_count": len(loaded.nodes),
            "marker_count": len(loaded.markers),
            "input_count": len(loaded.inputs),
            "output_count": len(loaded.outputs),
            "layout": {
                "present": bool(layout),
                "node_entries": len(layout.nodes) if layout else 0,
                "marker_entries": len(layout.markers) if layout else 0,
                "groups": len(layout.groups) if layout else 0,
            },
            "layout_attached": {
                "nodes": sum(1 for node in loaded.nodes.values() if node.layout),
                "markers": sum(
                    1 for marker in loaded.markers.values() if marker.layout
                ),
            },
            "warnings": loaded.warnings,
        },
    )
