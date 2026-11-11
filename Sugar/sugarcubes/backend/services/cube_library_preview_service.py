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
"""Build Cube browser preview, alias, and icon responses."""

from __future__ import annotations

import logging
from collections.abc import Callable, Collection
from pathlib import Path
from typing import Any, Mapping

from ...importer import CubeImportError
from ...instrumentation import log_event
from ..responses import BackendError
from .cube_file_io import format_display_path, read_cube_payload, safe_relative_path
from .cube_icon_service import (
    CubeIconError,
    attach_icon_url,
    normalize_existing_icon_metadata,
    normalize_icon_metadata,
    resolve_icon_asset_path,
)
from .cube_library_source_resolver import CubeLibrarySourceResolver
from .cube_metadata import normalize_metadata_string
from .cube_summary import (
    build_cube_identity_fields,
    dedupe_warnings,
    derive_cube_display_name,
)

_logger = logging.getLogger(__name__)


class CubeLibraryPreviewService:
    """Own read-only Cube presentation workflows."""

    def __init__(
        self,
        extension_root: Path,
        *,
        load_cube_artifact: Callable[[Path], Any],
        prepare_cube_import: Callable[..., Any],
        sources: CubeLibrarySourceResolver,
    ) -> None:
        """Initialize previews against stable import and source boundaries."""

        self.extension_root = extension_root.resolve()
        self.load_cube_artifact = load_cube_artifact
        self.prepare_cube_import = prepare_cube_import
        self.sources = sources

    def build_default_alias_lookup(self, cube_ids: Collection[str]) -> dict[str, str]:
        """Build a cube-id to display-name lookup for export flows."""

        lookup: dict[str, str] = {}
        for cube_id in cube_ids:
            normalized = normalize_metadata_string(cube_id)
            if not normalized:
                continue
            try:
                path = self.sources.resolve_cube_by_id(normalized)
            except BackendError as exc:
                if exc.status == 404:
                    continue
                raise
            payload, error = read_cube_payload(path)
            name = (
                derive_cube_display_name(payload, path.stem)
                if not error
                else normalize_metadata_string(path.stem)
            )
            if not name:
                continue
            lookup[normalized] = name
        return lookup

    def preview_cube(self, cube_id: str) -> dict[str, Any]:
        """Return the lightweight preview payload used by the cube browser."""

        cube_path = self.sources.resolve_cube_by_id(cube_id)
        try:
            loaded_cube = self.load_cube_artifact(cube_path)
            prepared = self.prepare_cube_import(loaded_cube, drop_origin=(0.0, 0.0))
        except CubeImportError:
            raise
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception("SugarCubes: preview failed for cube '%s'", cube_id)
            raise BackendError("Preview failed", status=500) from exc

        layout = loaded_cube.layout
        collapsed_nodes = 0
        styled_nodes = 0
        if layout:
            for entry in layout.nodes.values():
                if entry.extra.get("collapsed"):
                    collapsed_nodes += 1
                if any(key in entry.extra for key in ("color", "bgcolor", "style")):
                    styled_nodes += 1

        layout_summary = {
            "present": bool(layout),
            "groups": len(layout.groups) if layout else 0,
            "collapsed_nodes": collapsed_nodes,
            "styled_nodes": styled_nodes,
            "ds": layout.ds if layout else {"scale": 1.0, "offset": [0.0, 0.0]},
        }
        stats = {
            "nodes": len(loaded_cube.nodes),
            "markers": len(loaded_cube.markers),
            "inputs": len(loaded_cube.inputs),
            "outputs": len(loaded_cube.outputs),
            "definitions": len(loaded_cube.definitions),
            "prepared_nodes": len(prepared.nodes),
            "prepared_markers": len(prepared.markers),
            "connections": len(prepared.connections),
        }
        warnings = dedupe_warnings(list(loaded_cube.warnings) + list(prepared.warnings))
        source = self.sources.resolve_source_descriptor_by_path(cube_path)
        base_dir = Path(source["base_dir"])
        icon = attach_icon_url(
            normalize_existing_icon_metadata(loaded_cube.metadata.get("icon")),
            loaded_cube.cube_id,
        )

        log_event(
            "frontend.phase5",
            "preview_cube",
            {
                "path": safe_relative_path(cube_path, base_dir)
                or format_display_path(cube_path, self.extension_root),
                "node_count": stats["nodes"],
                "marker_count": stats["markers"],
                "layout_present": layout_summary["present"],
            },
        )
        return {
            "cube": {
                "name": cube_path.stem,
                **build_cube_identity_fields(
                    cube_id=loaded_cube.cube_id,
                    default_alias=normalize_metadata_string(
                        loaded_cube.metadata.get("default_alias")
                    )
                    or cube_path.stem,
                    metadata=loaded_cube.metadata,
                ),
                "description": loaded_cube.description,
                "metadata": loaded_cube.metadata,
                "icon": icon,
                "cube_id": loaded_cube.cube_id,
                "version": loaded_cube.version,
            },
            "stats": stats,
            "layout": layout_summary,
            "warnings": warnings,
            "source": {
                "path": format_display_path(cube_path, self.extension_root),
                "relative_path": safe_relative_path(cube_path, base_dir),
                "type": source["source_kind"],
                "owner": source["owner"],
                "repo": source["repo"],
                "repo_ref": source["repo_ref"],
                "namespace": source["namespace"],
            },
        }

    def resolve_cube_icon_asset(self, cube_id: str) -> tuple[Path, str]:
        """Return the resolved icon file and media type for one cube."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        if not normalized_cube_id:
            raise BackendError("'cube_id' query parameter is required", status=400)

        cube_path = self.sources.resolve_cube_by_id(normalized_cube_id)
        payload, error = read_cube_payload(cube_path)
        if error or not payload:
            raise BackendError(error or "Invalid cube payload", status=400)
        metadata = payload.get("metadata")
        icon_source = metadata.get("icon") if isinstance(metadata, Mapping) else None
        try:
            icon = normalize_icon_metadata(icon_source)
        except CubeIconError as exc:
            raise BackendError(str(exc), status=404) from exc
        if not icon:
            raise BackendError("Cube icon not found", status=404)

        source = self.sources.resolve_source_descriptor_by_path(cube_path)
        try:
            icon_path = resolve_icon_asset_path(Path(source["base_dir"]), icon)
        except CubeIconError as exc:
            raise BackendError(str(exc), status=404) from exc
        if not icon_path.exists() or not icon_path.is_file():
            raise BackendError("Cube icon asset not found", status=404)
        return icon_path, icon["media_type"]
