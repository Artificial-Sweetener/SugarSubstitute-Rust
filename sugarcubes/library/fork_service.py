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
"""Fork read-only embedded definitions without mutating their workflow source."""

from __future__ import annotations

import hashlib
from copy import deepcopy
from typing import Mapping, Protocol

from ..cube_model import parse_canonical_cube_id
from ..workflow import CanonicalWorkflow, EmbeddedCubeDefinition
from ..workflow.semantic_hash import semantic_hash
from .models import CubeForkRequest, CubeForkResult, CubeInstanceRebind


class ForkDefinitionRepository(Protocol):
    """Persist one validated fork before any graph rebind is exposed."""

    def persist(self, definition: EmbeddedCubeDefinition) -> None:
        """Persist a new writable definition or raise without side effects."""


class CubeForkService:
    """Own fork identity, lineage, persistence ordering, and rebind planning."""

    def __init__(self, repository: ForkDefinitionRepository) -> None:
        """Bind one destination-authorizing persistence port."""

        self._repository = repository

    def fork(
        self, workflow: CanonicalWorkflow, request: CubeForkRequest
    ) -> CubeForkResult:
        """Persist a derivative before returning an optional instance rebind patch."""

        source = workflow.definition_index().get(request.definition_id)
        if source is None:
            raise ValueError(
                f"Embedded Cube definition '{request.definition_id}' is unavailable"
            )
        if source.semantic_hash != request.expected_semantic_hash:
            raise ValueError("Embedded Cube definition changed after classification")
        if request.new_cube_id == source.cube_id:
            raise ValueError("Fork Cube identity must differ from its source")
        parsed = parse_canonical_cube_id(request.new_cube_id)
        if request.destination == "local" and parsed.source_kind != "local":
            raise ValueError("Local fork destination requires a local Cube identity")
        if request.destination == "authored_pack" and parsed.source_kind != "github":
            raise ValueError(
                "Authored-pack fork destination requires a GitHub Cube identity"
            )
        selected = _validate_selected_instances(
            workflow, source, request.selected_instance_ids
        )
        forked, lineage = _build_fork(source, request.new_cube_id)
        self._repository.persist(forked)
        rebinds = tuple(
            CubeInstanceRebind(instance_id, forked.definition_id, forked.cube_id)
            for instance_id in selected
        )
        return CubeForkResult(
            source_cube_id=source.cube_id,
            source_semantic_hash=source.semantic_hash,
            fork_cube_id=forked.cube_id,
            fork_semantic_hash=forked.semantic_hash,
            fork_definition_id=forked.definition_id,
            lineage=lineage,
            rebinds=rebinds,
        )


def _build_fork(
    source: EmbeddedCubeDefinition, new_cube_id: str
) -> tuple[EmbeddedCubeDefinition, Mapping[str, object]]:
    """Build a detached derivative with deterministic lineage and definition identity."""

    lineage: dict[str, object] = {
        "parent_cube_id": source.cube_id,
        "parent_cube_version": source.cube_version,
        "parent_semantic_hash": source.semantic_hash,
    }
    payload = deepcopy(dict(source.payload))
    extra = _require_mutable_mapping(payload.get("extra"), "fork definition extra")
    identity = _require_mutable_mapping(
        extra.get("sugarcubes_cube"), "fork Cube identity"
    )
    definition_id = _fork_definition_id(new_cube_id, source.semantic_hash)
    payload["id"] = definition_id
    identity["cube_id"] = new_cube_id
    provenance = identity.get("provenance")
    provenance_record = dict(provenance) if isinstance(provenance, Mapping) else {}
    provenance_record["lineage"] = lineage
    identity["provenance"] = provenance_record
    document = _fork_document(source, new_cube_id, lineage)
    if document is not None:
        extra["sugarcubes_document"] = document
    content_hash = semantic_hash(document or payload)
    return (
        EmbeddedCubeDefinition(
            definition_id=definition_id,
            cube_id=new_cube_id,
            cube_version=source.cube_version,
            default_alias=source.default_alias,
            semantic_hash=content_hash,
            provenance=provenance_record,
            payload=payload,
            document=document,
            native_subgraphs=source.native_subgraphs,
            native_node_definitions=source.native_node_definitions,
        ),
        lineage,
    )


def _fork_document(
    source: EmbeddedCubeDefinition,
    new_cube_id: str,
    lineage: Mapping[str, object],
) -> dict[str, object] | None:
    """Derive portable fork content when the source carries canonical content."""

    if source.document is None:
        return None
    document = deepcopy(dict(source.document))
    document["cube_id"] = new_cube_id
    metadata_value = document.get("metadata")
    metadata = dict(metadata_value) if isinstance(metadata_value, Mapping) else {}
    metadata["lineage"] = deepcopy(dict(lineage))
    document["metadata"] = metadata
    return document


def _validate_selected_instances(
    workflow: CanonicalWorkflow,
    source: EmbeddedCubeDefinition,
    selected_instance_ids: tuple[str, ...],
) -> tuple[str, ...]:
    """Require unique selected instances owned by the source definition."""

    selected = tuple(sorted(set(selected_instance_ids)))
    if len(selected) != len(selected_instance_ids):
        raise ValueError("Fork instance selection contains duplicates")
    available = {
        instance.instance_id
        for instance in workflow.instances
        if instance.definition_id == source.definition_id
    }
    unknown = tuple(
        instance_id for instance_id in selected if instance_id not in available
    )
    if unknown:
        raise ValueError(f"Fork instance selection is invalid: {', '.join(unknown)}")
    return selected


def _require_mutable_mapping(value: object, label: str) -> dict[str, object]:
    """Return a copied mutable record and retain it in the caller-owned payload."""

    if not isinstance(value, dict):
        raise ValueError(f"Embedded {label} is invalid")
    return value


def _fork_definition_id(cube_id: str, source_hash: str) -> str:
    """Build one deterministic native definition id from fork identity and lineage."""

    digest = hashlib.sha256(f"{cube_id}\0{source_hash}".encode("utf-8")).hexdigest()
    return f"sugarcube-{digest[:24]}"
