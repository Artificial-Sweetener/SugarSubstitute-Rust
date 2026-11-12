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
"""Persist prepared Cube exports and project authoritative save results."""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from typing import Any

from ...cube_model import (
    CubeDocument,
    CubeIdentityError,
    derive_route_from_cube_id,
    sanitize_authored_defaults_payload,
)
from ...exporter import ExportedCube
from ..responses import BackendError
from .cube_file_io import apply_cube_version
from .cube_implementation_defaults_service import (
    CubeImplementationDefaultsService,
    DefaultSaveDecision,
)
from .cube_library_service import CubeLibraryService
from .cube_metadata import (
    normalize_lineage_payload,
    normalize_metadata_string,
    normalize_metadata_update,
)
from .cube_save_commit_finalizer import CubeSaveCommitFinalizer
from .cube_save_models import CubeSaveCommitState, CubeSaveTarget, FinalizedCubeSave
from .cube_save_response import project_finalized_save
from .cube_save_target_planner import CubeSaveTargetPlanner

FinalizedDefinitionProvider = Callable[[Any, str, Mapping[str, Any]], Mapping[str, Any]]


class CubeExportPersistence:
    """Own export metadata, versioning, artifact writes, and result projection."""

    def __init__(
        self,
        library_service: CubeLibraryService,
        *,
        write_cubes_to_paths: Callable[..., Sequence[Mapping[str, Any]]],
        suggest_version: Callable[[Mapping[str, Any], Mapping[str, Any]], Any],
        finalized_definition_provider: FinalizedDefinitionProvider,
        implementation_defaults: CubeImplementationDefaultsService,
        targets: CubeSaveTargetPlanner,
        commits: CubeSaveCommitFinalizer,
    ) -> None:
        """Initialize persistence with explicit target, version, and commit owners."""

        self.library_service = library_service
        self.write_cubes_to_paths = write_cubes_to_paths
        self.suggest_version = suggest_version
        self.finalized_definition_provider = finalized_definition_provider
        self.implementation_defaults = implementation_defaults
        self.targets = targets
        self.commits = commits

    def finalize_exports(
        self,
        cubes: Sequence[ExportedCube],
        cube_entries: Mapping[str, Mapping[str, Any]],
        actor: Mapping[str, str],
    ) -> dict[str, Any]:
        """Apply source-safe metadata, write artifacts, and build the HTTP payload."""

        exported_lookup: dict[str, ExportedCube] = {}
        for exported in cubes:
            cube_id = normalize_metadata_string(exported.cube.get("cube_id"))
            if cube_id:
                exported_lookup[cube_id] = exported

        missing = set(cube_entries.keys()) - set(exported_lookup.keys())
        if missing:
            raise BackendError(
                "Cube ids missing from export",
                status=400,
                details={"missing": sorted(missing)},
            )

        version_suggestions: list[Mapping[str, str]] = []
        author_targets: list[CubeSaveTarget] = []
        commit_states: dict[str, CubeSaveCommitState] = {}
        for cube_id in cube_entries.keys():
            exported = exported_lookup[cube_id]
            entry = cube_entries[cube_id]
            self.apply_entry_metadata(exported, cube_id, entry, actor)

            sanitize_authored_defaults_payload(exported.cube)
            save_target = self.targets.build_author_save_target(
                cube_id=cube_id,
                exported=exported,
                previous_cube_id=normalize_metadata_string(
                    entry.get("previous_cube_id")
                ),
                forked=bool(entry.get("forked")),
                source_revision_ref=normalize_metadata_string(
                    entry.get("source_revision_ref")
                ),
                source_version=normalize_metadata_string(entry.get("source_version")),
                source_definition_key=normalize_metadata_string(
                    entry.get("source_definition_key")
                ),
                stale_save_mode=normalize_metadata_string(entry.get("stale_save_mode")),
            )
            author_targets.append(save_target)
            existing_payload = save_target.existing_payload
            self.implementation_defaults.apply(
                existing_payload=existing_payload,
                exported=exported,
                preserve_description=not bool(entry.get("description_set")),
                decision=DefaultSaveDecision.from_entry(entry),
            )
            if existing_payload is None:
                commit_states[cube_id] = self.commits.build_commit_state(
                    save_target=save_target,
                    previous_payload=None,
                )
                continue
            suggestion = self.suggest_version(existing_payload, exported.cube)
            current_version = normalize_metadata_string(exported.cube.get("version"))
            if save_target.stale_save_mode == "latest":
                apply_cube_version(exported.cube, suggestion.suggested)
                current_version = normalize_metadata_string(
                    exported.cube.get("version")
                )
            elif exported.version_auto:
                apply_cube_version(exported.cube, suggestion.suggested)
                current_version = normalize_metadata_string(
                    exported.cube.get("version")
                )
            elif current_version and current_version != suggestion.suggested:
                version_suggestions.append(
                    {
                        "default_alias": exported.default_alias,
                        "current_version": current_version,
                        "suggested_version": suggestion.suggested,
                        "reason": suggestion.reason,
                        "bump": suggestion.bump,
                    }
                )
            commit_states[cube_id] = self.commits.build_commit_state(
                save_target=save_target,
                previous_payload=existing_payload,
            )

        saved: list[Mapping[str, Any]] = []
        if author_targets:
            saved_author = self.write_cubes_to_paths(
                [
                    (save_target.exported, save_target.target_path)
                    for save_target in author_targets
                ],
                overwrite=True,
            )
            for saved_entry, save_target in zip(saved_author, author_targets):
                commit_state = commit_states.get(
                    save_target.cube_id
                ) or self.commits.build_commit_state(
                    save_target=save_target,
                    previous_payload=save_target.existing_payload,
                )
                finalized_commit_state = self.commits.finalize_commit(
                    save_target=save_target,
                    commit_state=commit_state,
                )
                document = CubeDocument.from_dict(save_target.exported.cube)
                definition = self.finalized_definition_provider(
                    save_target.target_path,
                    save_target.cube_id,
                    document.to_dict(),
                )
                finalized = FinalizedCubeSave(
                    target=save_target,
                    document=document,
                    artifact=dict(saved_entry),
                    commit_state=finalized_commit_state,
                    definition=dict(definition),
                )
                saved.append(project_finalized_save(finalized))

        warnings = [
            f"{cube.default_alias}: {warning}"
            for cube in cubes
            for warning in cube.warnings
            if warning
        ]
        response: dict[str, Any] = {"saved": saved}
        if warnings:
            response["warnings"] = warnings
        if version_suggestions:
            response["version_suggestions"] = version_suggestions
        if saved:
            self.library_service.notify_library_changed(
                affected_cube_ids=[
                    normalize_metadata_string(entry.get("cube_id"))
                    for entry in saved
                    if normalize_metadata_string(entry.get("cube_id"))
                ],
                saved_versions={
                    normalize_metadata_string(
                        entry.get("cube_id")
                    ): normalize_metadata_string(entry.get("version"))
                    for entry in saved
                    if normalize_metadata_string(entry.get("cube_id"))
                },
                reason="cube_saved",
            )
        return response

    def apply_entry_metadata(
        self,
        exported: ExportedCube,
        cube_id: str,
        entry: Mapping[str, Any],
        actor: Mapping[str, str],
    ) -> None:
        """Apply one save entry's catalog metadata to an export candidate."""

        if entry.get("description_set"):
            exported.cube["description"] = normalize_metadata_string(
                entry.get("description")
            )
        metadata = exported.cube.get("metadata")
        if not isinstance(metadata, dict):
            metadata = {}
            exported.cube["metadata"] = metadata
        entry_metadata = (
            entry.get("metadata") if isinstance(entry.get("metadata"), Mapping) else {}
        )
        if entry_metadata:
            updates, _removals = normalize_metadata_update(
                entry_metadata,
                cube_id=cube_id,
            )
            metadata.update(updates)
        try:
            metadata["default_alias"] = derive_route_from_cube_id(cube_id)
        except CubeIdentityError:
            pass
        if actor:
            if actor.get("author_url"):
                metadata["author_url"] = actor["author_url"]
            else:
                metadata.pop("author_url", None)
        if entry["forked"]:
            lineage = normalize_lineage_payload(entry.get("lineage"))
            if lineage:
                metadata["lineage"] = lineage
            else:
                metadata.pop("lineage", None)
