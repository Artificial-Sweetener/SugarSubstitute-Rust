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
"""Persist authored Cube flavors through versioning and repository policy."""

from __future__ import annotations

import json
from collections.abc import Callable, Mapping
from typing import Any, Optional

from ...cube_model import (
    CubeDocument,
    CubeSchemaError,
    compute_surface_signature,
    dedupe_flavor_id,
    normalize_flavor_id,
    sanitize_authored_defaults_payload,
)
from ...exporter import ExportedCube
from ..responses import BackendError
from .cube_file_io import apply_cube_version, read_cube_payload
from .cube_library_service import CubeLibraryService
from .cube_metadata import normalize_metadata_string
from .cube_save_commit_finalizer import CubeSaveCommitFinalizer
from .cube_save_models import CubeSaveTarget, FinalizedCubeSave
from .cube_save_response import project_finalized_save
from .cube_summary import derive_cube_display_name
from .local_flavor_service import LocalFlavorService

FinalizedDefinitionProvider = Callable[[Any, str, Mapping[str, Any]], Mapping[str, Any]]


class CubeAuthoredFlavorSaveService:
    """Own authored-flavor validation, persistence, versioning, and notification."""

    def __init__(
        self,
        library_service: CubeLibraryService,
        *,
        write_cube_to_path: Callable[..., Mapping[str, Any]],
        suggest_version: Callable[[Mapping[str, Any], Mapping[str, Any]], Any],
        finalized_definition_provider: FinalizedDefinitionProvider,
        commits: CubeSaveCommitFinalizer,
        local_flavor_service: Optional[LocalFlavorService],
    ) -> None:
        """Initialize authored-flavor saves with authoritative collaborators."""

        self.library_service = library_service
        self.write_cube_to_path = write_cube_to_path
        self.suggest_version = suggest_version
        self.finalized_definition_provider = finalized_definition_provider
        self.commits = commits
        self.local_flavor_service = local_flavor_service

    def save_authored_flavor(
        self,
        *,
        cube_id: str,
        values: Mapping[str, Any],
        flavor_id: str,
        flavor_name: str,
    ) -> dict[str, Any]:
        """Persist authored flavor values for one canonical cube file."""

        normalized_cube_id = normalize_metadata_string(cube_id)
        if not normalized_cube_id:
            raise BackendError("'cube_id' field is required", status=400)
        if not isinstance(values, Mapping):
            raise BackendError("'values' field is required", status=400)
        self.library_service.ownership_policy_service.assert_cube_id_writable(
            normalized_cube_id,
            action="save authored flavors into this cube",
        )

        cube_path = self.library_service.resolve_cube_by_id(normalized_cube_id)
        payload, error = read_cube_payload(cube_path)
        if error or not payload:
            raise BackendError(
                "Unable to read cube payload",
                status=500,
                details={"path": str(cube_path), "reason": error or "missing payload"},
            )
        try:
            document = CubeDocument.from_dict(payload)
        except CubeSchemaError as exc:
            raise BackendError(
                str(exc), status=400, details={"path": str(cube_path)}
            ) from exc

        next_document, saved_flavor_id = self._apply_authored_flavor_update(
            document=document,
            values=values,
            flavor_id=normalize_metadata_string(flavor_id),
            flavor_name=normalize_metadata_string(flavor_name),
        )
        self._reject_authored_local_flavor_collisions(
            cube_id=normalized_cube_id,
            document=next_document,
        )
        suggestion = self.suggest_version(document.to_dict(), next_document.to_dict())
        next_payload = next_document.to_dict()
        apply_cube_version(next_payload, suggestion.suggested)
        exported = ExportedCube(
            default_alias=derive_cube_display_name(payload, cube_path.stem),
            cube=next_payload,
            warnings=[],
            version_auto=False,
        )
        saved = self.write_cube_to_path(
            exported,
            cube_path,
            overwrite=True,
        )
        save_target = CubeSaveTarget(
            cube_id=normalized_cube_id,
            exported=exported,
            target_path=cube_path,
            existing_path=cube_path,
            existing_payload=payload,
            action_kind="update",
        )
        commit_state = self.commits.build_commit_state(
            save_target=save_target,
            previous_payload=payload,
        )
        finalized_commit_state = self.commits.finalize_commit(
            save_target=save_target,
            commit_state=commit_state,
        )
        persisted_document = CubeDocument.from_dict(next_payload)
        definition = self.finalized_definition_provider(
            cube_path,
            normalized_cube_id,
            persisted_document.to_dict(),
        )
        finalized = FinalizedCubeSave(
            target=save_target,
            document=persisted_document,
            artifact=dict(saved),
            commit_state=finalized_commit_state,
            definition=definition,
        )
        response = {
            "saved": {
                **project_finalized_save(finalized),
                "flavor_id": saved_flavor_id,
            }
        }
        self.library_service.notify_library_changed(
            affected_cube_ids=[normalized_cube_id],
            saved_versions={
                normalized_cube_id: normalize_metadata_string(next_payload["version"])
            },
            reason="authored_flavor_saved",
        )
        return response

    def _apply_authored_flavor_update(
        self,
        *,
        document: CubeDocument,
        values: Mapping[str, Any],
        flavor_id: str,
        flavor_name: str,
    ) -> tuple[CubeDocument, str]:
        """Return a document copy with one authored flavor created or updated."""

        control_ids = {control.control_id for control in document.surface.controls}
        unknown_controls = sorted(
            control_id for control_id in values.keys() if control_id not in control_ids
        )
        if unknown_controls:
            raise BackendError(
                "Flavor values reference unknown surface controls",
                status=400,
                details={"unknown_control_ids": unknown_controls},
            )

        authored = [flavor.to_dict() for flavor in document.flavors.authored]
        default_values = {
            str(control_id): json.loads(json.dumps(value))
            for control_id, value in values.items()
        }
        resolved_flavor_id = "default"
        if flavor_id == "default" or not flavor_id and not flavor_name:
            authored[0]["values"] = default_values
            authored[0]["name"] = "Default"
            resolved_flavor_id = "default"
        else:
            display_name = flavor_name or flavor_id
            if not display_name:
                raise BackendError(
                    "Flavor name is required for non-default authored flavors",
                    status=400,
                )
            existing = next(
                (
                    entry
                    for entry in authored
                    if normalize_metadata_string(entry.get("id")) == flavor_id
                ),
                None,
            )
            if existing is not None:
                existing["name"] = display_name
                existing["values"] = default_values
                resolved_flavor_id = normalize_metadata_string(existing.get("id"))
            else:
                used_ids = {
                    normalize_metadata_string(entry.get("id"))
                    for entry in authored
                    if entry.get("id")
                }
                used_ids.add("default")
                next_id = dedupe_flavor_id(
                    normalize_flavor_id(display_name),
                    used_ids,
                )
                authored.append(
                    {
                        "id": next_id,
                        "name": display_name,
                        "values": default_values,
                    }
                )
                resolved_flavor_id = next_id

        next_payload = document.to_dict()
        next_payload["flavors"]["authored"] = authored
        sanitize_authored_defaults_payload(next_payload)
        return CubeDocument.from_dict(next_payload), resolved_flavor_id

    def _reject_authored_local_flavor_collisions(
        self,
        *,
        cube_id: str,
        document: CubeDocument,
    ) -> None:
        """Reject authored flavor saves that would make local flavor names ambiguous."""

        if self.local_flavor_service is None:
            return
        authored = [flavor.to_dict() for flavor in document.flavors.authored]
        collisions = self.local_flavor_service.find_authored_local_collisions(
            cube_id=cube_id,
            surface_signature=compute_surface_signature(document.surface),
            authored_flavors=authored,
        )
        if not collisions:
            return
        raise BackendError(
            "Authored flavor collides with local flavor state",
            status=409,
            details={"collisions": collisions},
            extra={"code": "local_flavor_conflict"},
        )
