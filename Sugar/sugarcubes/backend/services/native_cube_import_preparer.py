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
"""Adapt in-memory Cube documents to existing prepared-import projection."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from typing import Any

from ...cube_model import CubeDocument
from ...cube_model.native_subgraph_defaults import (
    NativeSubgraphDefaultError,
    native_subgraph_defaults,
)
from ...importer import LoadedCube, PreparedImport

NodeDefinitionResolver = Callable[[str], Mapping[str, object] | None]


class NativeCubeImportPreparerAdapter:
    """Reuse the canonical importer for SugarScript-authored native Cubes."""

    def __init__(
        self,
        *,
        load_document: Callable[[CubeDocument], LoadedCube],
        prepare_import: Callable[[LoadedCube], PreparedImport],
        resolve_live_definition: NodeDefinitionResolver | None = None,
    ) -> None:
        """Bind pure in-memory importer collaborators."""

        self._load_document = load_document
        self._prepare_import = prepare_import
        self._resolve_live_definition = resolve_live_definition

    def prepare(self, document: CubeDocument) -> Mapping[str, object]:
        """Return the same JSON shape consumed by native picker placement."""

        prepared = self._prepare_import(self._load_document(document))
        nodes = [dict(node) for node in prepared.nodes]
        _apply_authored_nested_defaults(
            nodes,
            prepared.subgraphs,
            document.implementation.definitions,
            self._resolve_live_definition,
        )
        payload: dict[str, Any] = {
            "cube": dict(prepared.cube),
            "document": document.to_dict(),
            "nodes": nodes,
            "markers": list(prepared.markers),
            "connections": list(prepared.connections),
            "layout": dict(prepared.layout) if prepared.layout else None,
            "warnings": list(prepared.warnings),
            "subgraphs": list(prepared.subgraphs),
            "boundaries": dict(prepared.boundaries),
        }
        return payload


def _apply_authored_nested_defaults(
    nodes: list[dict[str, Any]],
    subgraphs: list[dict[str, Any]],
    definitions: Mapping[str, object],
    resolve_live_definition: NodeDefinitionResolver | None,
) -> None:
    """Apply Cube-authored wrapper defaults before script or workflow overrides."""

    subgraphs_by_id = {
        subgraph_id: subgraph
        for subgraph in subgraphs
        if isinstance(subgraph.get("id"), str)
        if (subgraph_id := str(subgraph["id"]).strip())
    }
    for node in nodes:
        class_type = node.get("class_type")
        subgraph = (
            subgraphs_by_id.get(class_type) if isinstance(class_type, str) else None
        )
        if subgraph is None:
            continue
        try:
            defaults = native_subgraph_defaults(
                subgraph,
                definitions,
                resolve_live_definition=resolve_live_definition,
            )
        except NativeSubgraphDefaultError as error:
            raise ValueError(
                f"Nested node '{node.get('symbol', class_type)}' defaults are invalid: {error}"
            ) from error
        inputs = node.setdefault("inputs", {})
        if not isinstance(inputs, dict):
            raise ValueError(
                f"Nested node '{node.get('symbol', class_type)}' inputs are invalid"
            )
        for input_name, value in defaults.items():
            if input_name not in inputs or inputs[input_name] is None:
                inputs[input_name] = value
