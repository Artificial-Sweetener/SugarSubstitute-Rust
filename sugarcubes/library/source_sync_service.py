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
"""Synchronize an approved claimed source and compare it without graph mutation."""

from __future__ import annotations

from typing import Literal, Protocol

from ..cube_model import parse_canonical_cube_id
from ..workflow import CanonicalWorkflow, EmbeddedCubeDefinition
from .models import CubeSourceSyncRequest, CubeSourceSyncResult


class CubeSourceSyncPort(Protocol):
    """Hide repository preflight, mutation, and exact artifact resolution."""

    def preflight(self, repo_ref: str) -> None:
        """Validate a claimed source without changing repository state."""

    def sync(self, repo_ref: str) -> None:
        """Track and synchronize one already approved repository."""

    def resolve_semantic_hash(
        self, *, repo_ref: str, cube_id: str, cube_version: str
    ) -> str | None:
        """Return the installed source hash for an exact identity/version claim."""


class CubeSourceSyncService:
    """Own approval, source derivation, synchronization order, and comparison."""

    def __init__(self, port: CubeSourceSyncPort) -> None:
        """Bind one repository infrastructure port."""

        self._port = port

    def sync(
        self, workflow: CanonicalWorkflow, request: CubeSourceSyncRequest
    ) -> CubeSourceSyncResult:
        """Synchronize only after approval and never replace embedded content."""

        definition = workflow.definition_index().get(request.definition_id)
        if definition is None:
            raise ValueError(
                f"Embedded Cube definition '{request.definition_id}' is unavailable"
            )
        if definition.semantic_hash != request.expected_semantic_hash:
            raise ValueError("Embedded Cube definition changed after classification")
        if not request.approved:
            raise PermissionError(
                "Cube source synchronization requires explicit approval"
            )
        repo_ref = _repo_ref(definition)
        self._port.preflight(repo_ref)
        self._port.sync(repo_ref)
        source_hash = self._port.resolve_semantic_hash(
            repo_ref=repo_ref,
            cube_id=definition.cube_id,
            cube_version=definition.cube_version,
        )
        status: Literal["exact", "divergent", "missing"] = (
            "missing"
            if source_hash is None
            else "exact" if source_hash == definition.semantic_hash else "divergent"
        )
        return CubeSourceSyncResult(
            status=status,
            repo_ref=repo_ref,
            embedded_semantic_hash=definition.semantic_hash,
            source_semantic_hash=source_hash,
        )


def _repo_ref(definition: EmbeddedCubeDefinition) -> str:
    """Resolve a canonical GitHub source from provenance or claimed identity."""

    provenance = definition.provenance
    if provenance is not None:
        value = provenance.get("repo_ref")
        if isinstance(value, str) and value.strip():
            parts = value.strip().split("/")
            if len(parts) != 2:
                raise ValueError("Cube provenance repo_ref is invalid")
            parsed = parse_canonical_cube_id(f"{parts[0]}/{parts[1]}/source.cube")
            return parsed.repo_ref
    parsed = parse_canonical_cube_id(definition.cube_id)
    if parsed.source_kind != "github":
        raise ValueError("Embedded Cube has no trackable GitHub source")
    return parsed.repo_ref
