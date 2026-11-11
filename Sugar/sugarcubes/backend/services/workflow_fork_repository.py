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
"""Persist workflow-derived forks through existing Cube Library owners."""

from __future__ import annotations

from ...cube_model import CubeDocument, CubeSchemaError
from ...workflow import EmbeddedCubeDefinition
from ..responses import BackendError
from .cube_artifact_repository import CubeArtifactRepository
from .cube_library_service import CubeLibraryService
from .ownership_policy_service import OwnershipPolicyService


class WorkflowForkRepository:
    """Own writable-destination validation and atomic fork persistence."""

    def __init__(
        self,
        *,
        artifacts: CubeArtifactRepository,
        ownership: OwnershipPolicyService,
        library: CubeLibraryService,
    ) -> None:
        """Bind existing source, access, and catalog-state owners."""

        self._artifacts = artifacts
        self._ownership = ownership
        self._library = library

    def persist(self, definition: EmbeddedCubeDefinition) -> None:
        """Persist a portable fork after identity, access, and collision checks."""

        if definition.document is None:
            raise BackendError(
                "This embedded Cube predates portable fork content",
                status=409,
            )
        try:
            document = CubeDocument.from_dict(definition.document)
        except CubeSchemaError as exc:
            raise BackendError("Fork Cube document is invalid", status=400) from exc
        if document.cube_id != definition.cube_id:
            raise BackendError("Fork Cube identity is inconsistent", status=400)
        self._ownership.assert_cube_id_writable(
            definition.cube_id,
            action="fork embedded Cube",
        )
        context = self._artifacts.assert_available(definition.cube_id)
        self._artifacts.write(context, document.to_dict())
        self._library.invalidate_catalog_state(
            reason="workflow_cube_forked",
            affected_cube_ids=(definition.cube_id,),
        )
