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
"""Model library matches without conflating storage, identity, and access."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal, Mapping

CubeLibraryClass = Literal["none", "captured", "local", "synced"]
CubeAccess = Literal["read_only", "writable"]
CubeCatalogMatchState = Literal["exact", "divergent"]


@dataclass(frozen=True)
class CatalogCubeArtifact:
    """Describe one machine-local library artifact available for comparison."""

    cube_id: str
    cube_version: str
    semantic_hash: str
    library_class: Literal["captured", "local", "synced"]
    access: CubeAccess
    source_ref: str


@dataclass(frozen=True)
class CubeLibraryMatch:
    """Report how one installed artifact compares with embedded workflow truth."""

    state: CubeCatalogMatchState
    library_class: Literal["captured", "local", "synced"]
    access: CubeAccess
    semantic_hash: str
    source_ref: str


@dataclass(frozen=True)
class CubeDefinitionClassification:
    """Report machine-derived library state for one embedded Cube definition."""

    definition_id: str
    cube_id: str
    cube_version: str
    semantic_hash: str
    instance_ids: tuple[str, ...]
    primary_class: CubeLibraryClass
    access: CubeAccess
    matches: tuple[CubeLibraryMatch, ...]
    divergent_matches: tuple[CubeLibraryMatch, ...]
    source_available: bool
    permitted_operations: frozenset[str]


@dataclass(frozen=True)
class CubeLibraryClassReport:
    """Collect deterministic classifications for a canonical workflow."""

    definitions: tuple[CubeDefinitionClassification, ...]


@dataclass(frozen=True)
class CaptureCubeRequest:
    """Identify exact embedded content requested for capture."""

    definition_id: str
    expected_semantic_hash: str


@dataclass(frozen=True)
class CaptureCubeResult:
    """Return unchanged identity and refreshed classification after capture."""

    cube_id: str
    cube_version: str
    semantic_hash: str
    provenance: Mapping[str, object] | None
    created: bool
    classification: CubeDefinitionClassification


@dataclass(frozen=True)
class CubeForkRequest:
    """Request one explicit derivation into a writable destination."""

    definition_id: str
    expected_semantic_hash: str
    new_cube_id: str
    selected_instance_ids: tuple[str, ...]
    destination: Literal["local", "authored_pack"]


@dataclass(frozen=True)
class CubeInstanceRebind:
    """Describe one optional graph rebind produced after successful persistence."""

    instance_id: str
    definition_id: str
    cube_id: str


@dataclass(frozen=True)
class CubeForkResult:
    """Return the persisted derivative and explicit lineage/rebind information."""

    source_cube_id: str
    source_semantic_hash: str
    fork_cube_id: str
    fork_semantic_hash: str
    fork_definition_id: str
    lineage: Mapping[str, object]
    rebinds: tuple[CubeInstanceRebind, ...]


@dataclass(frozen=True)
class CubeSourceSyncRequest:
    """Request explicitly approved synchronization of one claimed home source."""

    definition_id: str
    expected_semantic_hash: str
    approved: bool


@dataclass(frozen=True)
class CubeSourceSyncResult:
    """Report exact, divergent, or missing source content without replacement."""

    status: Literal["exact", "divergent", "missing"]
    repo_ref: str
    embedded_semantic_hash: str
    source_semantic_hash: str | None
