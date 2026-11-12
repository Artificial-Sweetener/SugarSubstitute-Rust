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
"""Own local Cube import and deletion mutations."""

from __future__ import annotations

import json
import logging
import shutil
from collections.abc import Callable
from pathlib import Path
from typing import Any

from ...cube_model import (
    CubeIdentityError,
    apply_cube_identity_projection,
    parse_canonical_cube_id,
)
from ...importer import CubeImportError
from ...instrumentation import log_event
from ..responses import BackendError
from .cube_file_io import (
    cleanup_failed_import,
    format_display_path,
    read_cube_payload,
    safe_relative_path,
)
from .cube_library_source_resolver import CubeLibrarySourceResolver
from .cube_metadata import normalize_metadata_string
from .ownership_policy_service import OwnershipPolicyService

_logger = logging.getLogger(__name__)


class CubeLibraryMutationService:
    """Own validated local Cube artifact mutations."""

    def __init__(
        self,
        extension_root: Path,
        *,
        load_cube_artifact: Callable[[Path], Any],
        ownership_policy_service: OwnershipPolicyService,
        sources: CubeLibrarySourceResolver,
        summarize_cube: Callable[[Path], dict[str, Any]],
        invalidate_catalog: Callable[..., None],
    ) -> None:
        """Initialize mutations with authoritative validation and state callbacks."""

        self.extension_root = extension_root.resolve()
        self.load_cube_artifact = load_cube_artifact
        self.ownership_policy_service = ownership_policy_service
        self.sources = sources
        self.summarize_cube = summarize_cube
        self.invalidate_catalog_state = invalidate_catalog

    def import_cube_file(
        self,
        *,
        source_value: str,
        target_cube_id: str,
        overwrite: bool,
    ) -> dict[str, Any]:
        """Copy one external `.cube` file into a canonical managed source location."""

        source_path = Path(source_value).expanduser()
        if not source_path.exists() or not source_path.is_file():
            raise BackendError(f"Source cube '{source_value}' not found", status=404)
        if source_path.suffix.lower() != ".cube":
            raise BackendError("Source must be a .cube file", status=400)

        normalized_target_cube_id = normalize_metadata_string(target_cube_id)
        if not normalized_target_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        try:
            parsed = parse_canonical_cube_id(normalized_target_cube_id)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        self.ownership_policy_service.assert_cube_id_writable(
            normalized_target_cube_id,
            action="import a cube into that destination",
        )

        resolved_dir = self.sources.resolve_source_base_dir(parsed)
        resolved_dir.mkdir(parents=True, exist_ok=True)
        dest_path = (resolved_dir / Path(parsed.path)).resolve()
        try:
            dest_path.relative_to(resolved_dir)
        except ValueError as exc:
            raise BackendError("Invalid destination", status=400) from exc
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        if dest_path.exists() and not overwrite:
            raise BackendError(
                f"Cube '{normalized_target_cube_id}' already exists", status=409
            )

        try:
            shutil.copy2(source_path, dest_path)
        except OSError as exc:
            _logger.exception(
                "SugarCubes: failed to import cube from %s to %s",
                source_path,
                dest_path,
            )
            raise BackendError("Failed to import cube", status=500) from exc

        payload, error = read_cube_payload(dest_path)
        if payload and not error:
            payload_dict = dict(payload)
            previous_cube_id = normalize_metadata_string(payload_dict.get("cube_id"))
            payload_dict["cube_id"] = normalized_target_cube_id
            try:
                apply_cube_identity_projection(
                    payload_dict, previous_cube_id=previous_cube_id
                )
                with dest_path.open("w", encoding="utf-8") as handle:
                    json.dump(payload_dict, handle, indent=2)
                    handle.write("\n")
            except (OSError, TypeError, ValueError) as exc:
                cleanup_failed_import(dest_path)
                _logger.exception(
                    "SugarCubes: failed to persist imported cube identity for %s",
                    dest_path,
                )
                raise BackendError("Failed to import cube", status=500) from exc

        try:
            self.load_cube_artifact(dest_path)
        except CubeImportError:
            cleanup_failed_import(dest_path)
            raise
        except Exception as exc:
            cleanup_failed_import(dest_path)
            _logger.exception(
                "SugarCubes: imported cube failed validation for %s",
                dest_path,
            )
            raise BackendError("Imported cube failed validation", status=500) from exc

        log_event(
            "frontend.phase5",
            "import_cube_file",
            {
                "source": str(source_path.name),
                "dest": safe_relative_path(dest_path, resolved_dir)
                or normalized_target_cube_id,
            },
        )
        self.invalidate_catalog_state(
            reason="cube_imported",
            affected_cube_ids=[normalized_target_cube_id],
        )
        return {"cube": self.summarize_cube(dest_path)}

    def delete_cube(
        self,
        *,
        cube_id: str,
    ) -> dict[str, Any]:
        """Delete a tracked cube by canonical id."""

        self.ownership_policy_service.assert_cube_id_writable(
            cube_id,
            action="delete this cube",
        )
        cube_path = self.sources.resolve_cube_by_id(cube_id)

        try:
            cube_path.unlink()
        except FileNotFoundError as exc:
            raise BackendError("Cube already removed", status=404) from exc
        except OSError as exc:
            _logger.exception("SugarCubes: failed to delete cube %s", cube_path)
            raise BackendError("Failed to delete cube", status=500) from exc

        source = self.sources.resolve_source_descriptor_by_path(cube_path)
        base_dir = Path(source["base_dir"])
        log_event(
            "frontend.phase5",
            "delete_cube",
            {
                "path": safe_relative_path(cube_path, base_dir)
                or format_display_path(cube_path, self.extension_root)
            },
        )
        self.invalidate_catalog_state(
            reason="cube_deleted", affected_cube_ids=[cube_id]
        )
        return {
            "status": "deleted",
            "cube": format_display_path(cube_path, self.extension_root),
        }
