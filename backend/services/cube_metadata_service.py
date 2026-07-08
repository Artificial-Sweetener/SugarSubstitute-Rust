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
"""Cube metadata mutation services for source-qualified SugarCubes."""

from __future__ import annotations

import json
import logging
from pathlib import Path
from typing import Any, Callable, Mapping

try:
    from ...cube_model import (
        CubeIdentityError,
        apply_cube_identity_projection,
        derive_cube_id_from_route,
        derive_route_from_cube_id,
        validate_cube_route_identity,
    )
    from ...instrumentation import log_event
    from ..responses import BackendError
    from .cube_library_service import (
        CubeLibraryService,
        normalize_default_alias,
        normalize_metadata_string,
        normalize_metadata_update,
        read_cube_payload,
        safe_relative_path,
    )
except ImportError:
    from cube_model import (
        CubeIdentityError,
        apply_cube_identity_projection,
        derive_cube_id_from_route,
        derive_route_from_cube_id,
        validate_cube_route_identity,
    )
    from instrumentation import log_event
    from backend.responses import BackendError
    from backend.services.cube_library_service import (
        CubeLibraryService,
        normalize_default_alias,
        normalize_metadata_string,
        normalize_metadata_update,
        read_cube_payload,
        safe_relative_path,
    )

_logger = logging.getLogger(__name__)


class CubeMetadataService:
    """Own cube rename and metadata update behavior."""

    def __init__(
        self,
        library_service: CubeLibraryService,
        *,
        retarget_cube_payload: Callable[..., None],
    ) -> None:
        """Initialize the metadata service."""

        self.library_service = library_service
        self.retarget_cube_payload = retarget_cube_payload

    def update_metadata(
        self,
        *,
        cube_id: str,
        description_set: bool,
        description: str,
        version_set: bool,
        version: str,
        metadata_payload: Mapping[str, Any],
    ) -> dict[str, Any]:
        """Update stored cube metadata, description, and version."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        if not normalized_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        if version_set and not version:
            raise BackendError("'version' must be a non-empty string", status=400)
        self.library_service.ownership_policy_service.assert_cube_id_writable(
            normalized_cube_id,
            action="update cube metadata",
        )

        cube_path = self.library_service.resolve_cube_by_id(normalized_cube_id)
        payload, error = read_cube_payload(cube_path)
        if error or not payload:
            raise BackendError(error or "Invalid cube payload", status=400)

        payload_cube_id = normalize_metadata_string(payload.get("cube_id"))
        if not payload_cube_id:
            raise BackendError("Cube payload is missing cube_id", status=400)
        if payload_cube_id != normalized_cube_id:
            raise BackendError(
                "Cube id mismatch",
                status=409,
                details={"expected": normalized_cube_id, "actual": payload_cube_id},
            )
        if not normalize_metadata_string(payload.get("version")):
            raise BackendError("Cube payload is missing version", status=400)

        updates, removals = normalize_metadata_update(
            metadata_payload,
            cube_id=normalized_cube_id,
        )
        metadata = payload.get("metadata")
        if not isinstance(metadata, Mapping):
            metadata = {}
        else:
            metadata = dict(metadata)

        for key, value in updates.items():
            metadata[key] = value
        for key in removals:
            metadata.pop(key, None)
        # Source identity owns the browser Author label; drop legacy free-form author.
        metadata.pop("author", None)

        if metadata:
            payload["metadata"] = metadata
        else:
            payload.pop("metadata", None)

        if description_set:
            payload["description"] = description
        if version_set:
            payload["version"] = version

        self._write_payload(cube_path, payload, "Failed to update cube metadata")
        source = self.library_service.resolve_source_descriptor_by_path(cube_path)
        base_dir = Path(source["base_dir"])
        log_event(
            "frontend.phase5",
            "update_metadata",
            {"path": safe_relative_path(cube_path, base_dir) or cube_path.name},
        )
        self.library_service.invalidate_catalog_state(
            reason="cube_metadata_updated",
            affected_cube_ids=[normalized_cube_id],
        )
        return {"cube": self.library_service.summarize_cube(cube_path)}

    def rename_cube(
        self,
        *,
        cube_id: str,
        target_cube_id: str,
        target_default_alias: str,
    ) -> dict[str, Any]:
        """Move a cube to a new canonical identity and update embedded references."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        normalized_target_cube_id = normalize_metadata_string(target_cube_id)
        normalized_target_default_alias = normalize_default_alias(target_default_alias)
        if not normalized_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        if not normalized_target_cube_id:
            raise BackendError("'target_cube_id' field is required", status=400)
        try:
            resolved_route_alias = (
                normalized_target_default_alias
                or derive_route_from_cube_id(normalized_target_cube_id)
            )
            validate_cube_route_identity(
                normalized_target_cube_id, resolved_route_alias
            )
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        self.library_service.ownership_policy_service.assert_cube_id_writable(
            normalized_cube_id,
            action="rename this cube",
        )
        self.library_service.ownership_policy_service.assert_cube_id_writable(
            normalized_target_cube_id,
            action="rename this cube into that destination",
        )

        cube_path = self.library_service.resolve_cube_by_id(normalized_cube_id)
        target_path = self.library_service.resolve_cube_target_path(
            normalized_target_cube_id
        )
        payload, error = read_cube_payload(cube_path)
        if error or not payload:
            raise BackendError(error or "Invalid cube payload", status=400)

        payload_cube_id = normalize_metadata_string(payload.get("cube_id"))
        if payload_cube_id != normalized_cube_id:
            raise BackendError(
                "Cube id mismatch",
                status=409,
                details={"expected": normalized_cube_id, "actual": payload_cube_id},
            )

        existing_default_alias = normalize_metadata_string(
            (payload.get("metadata") or {}).get("default_alias")
            if isinstance(payload.get("metadata"), Mapping)
            else ""
        )
        try:
            previous_default_alias = (
                existing_default_alias or derive_route_from_cube_id(normalized_cube_id)
            )
        except CubeIdentityError:
            previous_default_alias = existing_default_alias or cube_path.stem
        resolved_target_default_alias = resolved_route_alias

        if normalized_cube_id == normalized_target_cube_id:
            return {"cube": self.library_service.summarize_cube(cube_path)}

        if target_path.exists() and target_path.resolve() != cube_path.resolve():
            raise BackendError(
                f"Cube '{normalized_target_cube_id}' already exists", status=409
            )

        payload["cube_id"] = normalized_target_cube_id
        metadata = payload.get("metadata")
        if not isinstance(metadata, Mapping):
            metadata_dict: dict[str, Any] = {}
        else:
            metadata_dict = dict(metadata)
        metadata_dict["default_alias"] = resolved_target_default_alias
        payload["metadata"] = metadata_dict
        self.retarget_cube_payload(
            payload,
            previous_cube_id=normalized_cube_id,
            target_cube_id=normalized_target_cube_id,
            previous_default_alias=previous_default_alias,
            target_default_alias=resolved_target_default_alias,
        )

        if target_path.resolve() == cube_path.resolve():
            self._write_payload(
                cube_path,
                payload,
                "Failed to update cube identity",
                previous_cube_id=normalized_cube_id,
            )
        else:
            target_path.parent.mkdir(parents=True, exist_ok=True)
            self._write_payload(
                target_path,
                payload,
                "Failed to update cube identity",
                previous_cube_id=normalized_cube_id,
            )
            try:
                cube_path.unlink()
            except OSError as exc:
                try:
                    target_path.unlink()
                except OSError:
                    _logger.warning(
                        "SugarCubes: failed to clean up target cube after source removal error",
                        exc_info=True,
                    )
                _logger.exception(
                    "SugarCubes: failed to remove renamed cube source %s after writing %s",
                    cube_path,
                    target_path,
                )
                raise BackendError("Failed to finalize cube move", status=500) from exc

        log_event(
            "frontend.phase5",
            "rename_cube",
            {
                "from": normalized_cube_id,
                "to": normalized_target_cube_id,
            },
        )
        self.library_service.invalidate_catalog_state(
            reason="cube_renamed",
            affected_cube_ids=[normalized_cube_id, normalized_target_cube_id],
        )
        return {"cube": self.library_service.summarize_cube(target_path)}

    def rename_cube_from_default_alias(
        self,
        *,
        cube_id: str,
        target_default_alias: str,
    ) -> dict[str, Any]:
        """Move a cube by deriving the target identity from Default Alias."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        normalized_target_default_alias = normalize_default_alias(target_default_alias)
        if not normalized_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        if not normalized_target_default_alias:
            raise BackendError("'default_alias' field is required", status=400)
        try:
            target_cube_id = derive_cube_id_from_route(
                source_cube_id=normalized_cube_id,
                route=normalized_target_default_alias,
            )
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        return self.rename_cube(
            cube_id=normalized_cube_id,
            target_cube_id=target_cube_id,
            target_default_alias=normalized_target_default_alias,
        )

    def _write_payload(
        self,
        cube_path: Path,
        payload: Mapping[str, Any],
        failure_message: str,
        *,
        previous_cube_id: str = "",
    ) -> None:
        """Persist a cube payload back to disk."""

        try:
            payload_dict = dict(payload)
            apply_cube_identity_projection(
                payload_dict, previous_cube_id=previous_cube_id
            )
            with cube_path.open("w", encoding="utf-8") as handle:
                json.dump(payload_dict, handle, indent=2)
                handle.write("\n")
        except (OSError, TypeError, ValueError) as exc:
            _logger.exception(
                "SugarCubes: failed to persist cube payload for %s",
                cube_path,
            )
            raise BackendError(failure_message, status=500) from exc
