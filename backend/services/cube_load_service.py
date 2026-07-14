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
"""Cube loading service for SugarCubes."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any, Callable, Sequence

try:
    from ...importer import CubeImportError
    from ...instrumentation import log_diagnostic
    from ..responses import BackendError
    from .cube_icon_service import attach_icon_url, normalize_existing_icon_metadata
    from .cube_identity_redirect_service import CubeIdentityRedirectService
    from .cube_library_service import (
        CubeLibraryService,
        build_cube_identity_fields,
        normalize_metadata_string,
    )
except ImportError:
    from importer import CubeImportError
    from instrumentation import log_diagnostic
    from backend.responses import BackendError
    from backend.services.cube_library_service import (
        CubeLibraryService,
        build_cube_identity_fields,
        normalize_metadata_string,
    )
    from backend.services.cube_icon_service import (
        attach_icon_url,
        normalize_existing_icon_metadata,
    )
    from backend.services.cube_identity_redirect_service import (
        CubeIdentityRedirectService,
    )

_logger = logging.getLogger(__name__)
CUBE_LOAD_TRACE_MARKER = "SugarCubes cube load diagnostic"


class CubeLoadService:
    """Own cube artifact loading and prepared import responses."""

    def __init__(
        self,
        library_service: CubeLibraryService,
        *,
        load_cube_artifact: Callable[[Any], Any],
        prepare_cube_import: Callable[..., Any],
        redirect_service: CubeIdentityRedirectService,
    ) -> None:
        """Initialize the cube load service."""

        self.library_service = library_service
        self.load_cube_artifact = load_cube_artifact
        self.prepare_cube_import = prepare_cube_import
        self.redirect_service = redirect_service

    def load_cube(
        self,
        *,
        cube_id: str,
        version_pin: str,
        drop_origin: Sequence[float],
    ) -> dict[str, Any]:
        """Return the prepared cube import payload for the frontend."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        if not normalized_cube_id:
            raise BackendError("'cube_id' field is required", status=400)

        resolved_cube_id = self.redirect_service.resolve(normalized_cube_id)
        cube_path = self.library_service.resolve_cube_by_id(resolved_cube_id)
        result = self.load_cube_path(
            cube_path=cube_path,
            cube_id=resolved_cube_id,
            version_pin=version_pin,
            drop_origin=drop_origin,
        )
        if resolved_cube_id != normalized_cube_id:
            result["identity_redirect"] = {
                "requested_cube_id": normalized_cube_id,
                "resolved_cube_id": resolved_cube_id,
            }
        return result

    def load_cube_path(
        self,
        *,
        cube_path: Path,
        cube_id: str,
        version_pin: str = "",
        drop_origin: Sequence[float] = (0.0, 0.0),
    ) -> dict[str, Any]:
        """Return the canonical prepared definition for one persisted cube path."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        if not normalized_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        _log_cube_library_diagnostic(
            "sugarcubes_frontend_load_cube_start",
            cube_id=normalized_cube_id,
            version_pin=version_pin,
        )
        try:
            loaded_cube = self.load_cube_artifact(cube_path)
            if version_pin and loaded_cube.version != version_pin:
                _log_cube_library_diagnostic(
                    "sugarcubes_frontend_load_cube_version_pin_mismatch",
                    cube_id=normalized_cube_id,
                    expected_version=version_pin,
                    actual_version=loaded_cube.version,
                )
                raise BackendError(
                    "Cube version mismatch",
                    status=409,
                    details={"expected": version_pin, "actual": loaded_cube.version},
                )
            prepared = self.prepare_cube_import(loaded_cube, drop_origin=drop_origin)
        except CubeImportError:
            raise
        except BackendError:
            raise
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception(
                "SugarCubes load failed for cube '%s'", normalized_cube_id
            )
            raise BackendError("Load failed", status=500) from exc

        cube_payload = dict(prepared.cube)
        cube_payload.setdefault("name", cube_path.stem)
        metadata = (
            cube_payload.get("metadata")
            if isinstance(cube_payload.get("metadata"), dict)
            else {}
        )
        icon = attach_icon_url(
            normalize_existing_icon_metadata(metadata.get("icon")),
            normalize_metadata_string(cube_payload.get("cube_id")),
        )
        if icon:
            cube_payload["icon"] = icon
        cube_payload.update(
            build_cube_identity_fields(
                cube_id=normalize_metadata_string(cube_payload.get("cube_id"))
                or normalized_cube_id,
                default_alias=normalize_metadata_string(metadata.get("default_alias"))
                or normalize_metadata_string(cube_payload.get("name"))
                or cube_path.stem,
                metadata=metadata,
            )
        )
        source = self.library_service.resolve_source_descriptor_by_path(cube_path)
        base_dir = Path(source["base_dir"])
        source_info = {
            "path": str(cube_path),
            "name": cube_path.stem,
            "type": source["source_kind"],
            "owner": source["owner"],
            "repo": source["repo"],
            "repo_ref": source["repo_ref"],
            "namespace": source["namespace"],
        }
        try:
            relative_path = cube_path.relative_to(base_dir)
        except ValueError:
            relative_path = None
        if relative_path is not None:
            source_info["relative_path"] = str(relative_path).replace("\\", "/")

        _log_cube_library_diagnostic(
            "sugarcubes_frontend_load_cube_return",
            cube_id=normalized_cube_id,
            loaded_cube_id=normalize_metadata_string(cube_payload.get("cube_id")),
            loaded_version=normalize_metadata_string(cube_payload.get("version")),
            source_type=source_info["type"],
            relative_path=source_info.get("relative_path", ""),
        )
        return {
            "cube": cube_payload,
            "nodes": prepared.nodes,
            "markers": prepared.markers,
            "connections": prepared.connections,
            "layout": prepared.layout,
            "warnings": prepared.warnings,
            "subgraphs": prepared.subgraphs,
            "source": source_info,
        }


def _log_cube_library_diagnostic(event: str, **fields: object) -> None:
    """Emit a structured cube load diagnostic line in standard Comfy logs."""

    log_diagnostic(_logger, CUBE_LOAD_TRACE_MARKER, event, fields)
