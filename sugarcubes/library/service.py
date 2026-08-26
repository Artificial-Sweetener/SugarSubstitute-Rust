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
"""Coordinate workflow classification and exact Stable preservation."""

from __future__ import annotations

from collections.abc import Callable, Iterable

from ..workflow import CanonicalWorkflow
from .classification import classify_workflow
from .models import (
    CatalogCubeArtifact,
    CubeLibraryClassReport,
    StableCubeSaveRequest,
    StableCubeSaveResult,
)
from .stable_repository import StableCubeRepository


class CubeLibraryClassService:
    """Own machine-derived classification without changing embedded definitions."""

    def __init__(
        self,
        *,
        stable: StableCubeRepository,
        catalog_artifacts: Callable[[], Iterable[CatalogCubeArtifact]] = tuple,
    ) -> None:
        """Bind explicit Stable and installed-catalog ports."""

        self._stable = stable
        self._catalog_artifacts = catalog_artifacts

    def classify_workflow(self, workflow: CanonicalWorkflow) -> CubeLibraryClassReport:
        """Classify workflow definitions from current machine state."""

        artifacts = (*self._catalog_artifacts(), *self._stable.list_artifacts())
        return classify_workflow(workflow, artifacts)

    def save_to_stable(
        self, workflow: CanonicalWorkflow, request: StableCubeSaveRequest
    ) -> StableCubeSaveResult:
        """Save exact expected content without forking identity or rebinding instances."""

        definition = workflow.definition_index().get(request.definition_id)
        if definition is None:
            raise ValueError(
                f"Embedded Cube definition '{request.definition_id}' is unavailable"
            )
        if definition.semantic_hash != request.expected_semantic_hash:
            raise ValueError("Embedded Cube definition changed after classification")
        write = self._stable.save(definition)
        refreshed = self.classify_workflow(workflow)
        classification = next(
            item
            for item in refreshed.definitions
            if item.definition_id == definition.definition_id
        )
        return StableCubeSaveResult(
            cube_id=definition.cube_id,
            cube_version=definition.cube_version,
            semantic_hash=definition.semantic_hash,
            provenance=definition.provenance,
            created=write.created,
            classification=classification,
        )
