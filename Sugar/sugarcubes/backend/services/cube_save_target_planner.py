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
"""Resolve authoritative managed paths and action kinds for Cube saves."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Mapping, Optional

from ...cube_model import CubeIdentityError, parse_canonical_cube_id
from ...exporter import ExportedCube
from ..responses import BackendError
from .cube_file_io import read_cube_payload
from .cube_library_service import CubeLibraryService
from .cube_save_models import CubeSaveTarget


class CubeSaveTargetPlanner:
    """Own source-safe save target and action planning."""

    def __init__(self, library_service: CubeLibraryService) -> None:
        """Initialize target planning against the authoritative Cube library."""

        self.library_service = library_service

    def build_author_save_target(
        self,
        *,
        cube_id: str,
        exported: ExportedCube,
        previous_cube_id: str = "",
        forked: bool = False,
        source_revision_ref: str = "",
        source_version: str = "",
        source_definition_key: str = "",
        stale_save_mode: str = "",
    ) -> CubeSaveTarget:
        """Resolve the authoritative tracked-repo write target for one author save."""

        self.library_service.ownership_policy_service.assert_cube_id_writable(
            cube_id,
            action="save into this cube target",
        )
        existing_path = self._find_existing_author_cube_path(cube_id)
        existing_payload: Optional[Mapping[str, Any]] = None
        if existing_path is not None:
            existing_payload, error = read_cube_payload(existing_path)
            if error or not existing_payload:
                existing_payload = None
        action_kind = self._derive_save_action_kind(
            cube_id=cube_id,
            previous_cube_id=previous_cube_id,
            existing_path=existing_path,
            forked=forked,
        )
        target_path = existing_path
        if target_path is None:
            target_path = self._build_target_path_from_cube_id(cube_id)
        return CubeSaveTarget(
            cube_id=cube_id,
            exported=exported,
            target_path=target_path,
            existing_path=existing_path,
            previous_cube_id=previous_cube_id,
            existing_payload=existing_payload,
            action_kind=action_kind,
            forked=forked,
            source_revision_ref=source_revision_ref,
            source_version=source_version,
            source_definition_key=source_definition_key,
            stale_save_mode=stale_save_mode,
        )

    def _find_existing_author_cube_path(self, cube_id: str) -> Optional[Path]:
        """Resolve an existing managed cube path for overwrite flows when present."""

        try:
            return self.library_service.resolve_cube_by_id(cube_id)
        except BackendError as exc:
            if exc.status == 404:
                return None
            raise

    def _build_target_path_from_cube_id(self, cube_id: str) -> Path:
        """Resolve a new tracked cube save path directly from canonical identity."""

        try:
            parsed = parse_canonical_cube_id(cube_id)
        except CubeIdentityError as exc:
            raise BackendError(str(exc), status=400) from exc
        base_dir = self.library_service.resolve_source_base_dir(parsed)
        target_path = (base_dir / Path(parsed.path)).resolve()
        try:
            target_path.relative_to(base_dir)
        except ValueError as exc:
            raise BackendError(
                "Cube id path must stay within the managed source", status=400
            ) from exc
        target_path.parent.mkdir(parents=True, exist_ok=True)
        return target_path

    def _derive_save_action_kind(
        self,
        *,
        cube_id: str,
        previous_cube_id: str,
        existing_path: Optional[Path],
        forked: bool,
    ) -> str:
        """Derive the save action used for commit-message generation."""

        if forked:
            return "fork"
        if previous_cube_id and previous_cube_id != cube_id:
            return "rename"
        if existing_path is not None:
            return "update"
        return "create"
