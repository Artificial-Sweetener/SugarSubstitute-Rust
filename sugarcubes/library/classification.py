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
"""Classify embedded definitions against optional machine-local artifacts."""

from __future__ import annotations

from collections import defaultdict
from typing import Iterable

from ..cube_model import CubeIdentityError, parse_canonical_cube_id
from ..workflow import CanonicalWorkflow, EmbeddedCubeDefinition
from .models import (
    CatalogCubeArtifact,
    CubeAccess,
    CubeDefinitionClassification,
    CubeLibraryClassReport,
    CubeLibraryMatch,
)

_CLASS_PRIORITY = {"local": 0, "stable": 1, "synced": 2}


def classify_workflow(
    workflow: CanonicalWorkflow,
    artifacts: Iterable[CatalogCubeArtifact],
) -> CubeLibraryClassReport:
    """Compare every embedded definition while preserving embedded authority."""

    candidates: dict[tuple[str, str], list[CatalogCubeArtifact]] = defaultdict(list)
    for artifact in artifacts:
        candidates[(artifact.cube_id, artifact.cube_version)].append(artifact)
    instance_ids: dict[str, list[str]] = defaultdict(list)
    for instance in workflow.instances:
        instance_ids[instance.definition_id].append(instance.instance_id)
    classifications = tuple(
        _classify_definition(
            definition,
            candidates.get((definition.cube_id, definition.cube_version), ()),
            tuple(sorted(instance_ids.get(definition.definition_id, ()))),
        )
        for definition in workflow.definitions
    )
    return CubeLibraryClassReport(definitions=classifications)


def _classify_definition(
    definition: EmbeddedCubeDefinition,
    candidates: Iterable[CatalogCubeArtifact],
    instance_ids: tuple[str, ...],
) -> CubeDefinitionClassification:
    """Classify one definition and derive allowed library operations."""

    exact: list[CubeLibraryMatch] = []
    divergent: list[CubeLibraryMatch] = []
    for artifact in candidates:
        match = CubeLibraryMatch(
            state=(
                "exact"
                if artifact.semantic_hash == definition.semantic_hash
                else "divergent"
            ),
            library_class=artifact.library_class,
            access=artifact.access,
            semantic_hash=artifact.semantic_hash,
            source_ref=artifact.source_ref,
        )
        (exact if match.state == "exact" else divergent).append(match)
    exact.sort(key=_match_order)
    divergent.sort(key=_match_order)
    primary_class = exact[0].library_class if exact else "none"
    access: CubeAccess = (
        "writable"
        if any(match.access == "writable" for match in (*exact, *divergent))
        else "read_only"
    )
    source_available = _source_available(definition)
    operations = {"keep", "fork"}
    if not any(match.library_class == "stable" for match in exact):
        operations.add("save_to_stable")
    if source_available:
        operations.add("track_source")
    if access == "writable":
        operations.add("edit_definition")
    return CubeDefinitionClassification(
        definition_id=definition.definition_id,
        cube_id=definition.cube_id,
        cube_version=definition.cube_version,
        semantic_hash=definition.semantic_hash,
        instance_ids=instance_ids,
        primary_class=primary_class,
        access=access,
        matches=tuple(exact),
        divergent_matches=tuple(divergent),
        source_available=source_available,
        permitted_operations=frozenset(operations),
    )


def _match_order(match: CubeLibraryMatch) -> tuple[int, str, str]:
    """Order all matches independently of catalog enumeration order."""

    return (_CLASS_PRIORITY[match.library_class], match.source_ref, match.semantic_hash)


def _source_available(definition: EmbeddedCubeDefinition) -> bool:
    """Derive whether a source can be tracked from untrusted optional hints."""

    provenance = definition.provenance
    if provenance is not None and any(
        isinstance(provenance.get(key), str) and bool(str(provenance[key]).strip())
        for key in ("repo_ref", "home", "source")
    ):
        return True
    try:
        return parse_canonical_cube_id(definition.cube_id).source_kind == "github"
    except CubeIdentityError:
        return False
