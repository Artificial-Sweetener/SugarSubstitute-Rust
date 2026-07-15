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
"""Cube export orchestration for SugarCubes."""

from __future__ import annotations

import json
import logging
import re
from copy import deepcopy
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Collection, Mapping, Optional, Sequence

from ...cube_model import (
    CubeIdentityError,
    CubeDocument,
    CubeSchemaError,
    dedupe_flavor_id,
    derive_route_from_cube_id,
    compute_surface_signature,
    normalize_flavor_id,
    parse_canonical_cube_id,
    preserve_authored_flavors_for_implementation_save,
    sanitize_authored_defaults_document,
    sanitize_authored_defaults_payload,
)
from ...exporter import CubeValidationError, ExportedCube
from ...exporter.graph import CubeAnalysis, analyze_cubes
from ..responses import BackendError
from .cube_git_context import CubeGitContext, resolve_cube_git_context
from .cube_file_io import apply_cube_version, read_cube_payload
from .cube_library_service import CubeLibraryService
from .cube_summary import derive_cube_display_name
from .cube_metadata import (
    normalize_lineage_payload,
    normalize_metadata_string,
    normalize_metadata_update,
)
from .tracked_repo_service import CubeCommitResult
from .local_flavor_service import LocalFlavorService

_logger = logging.getLogger(__name__)
_UUID_CLASS_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
    re.IGNORECASE,
)
NodeClassMappingsProvider = Callable[[], Mapping[str, Any]]
FinalizedDefinitionProvider = Callable[
    [Path, str, Mapping[str, Any]], Mapping[str, Any]
]


@dataclass(frozen=True)
class CubeSaveTarget:
    """Capture the resolved author-save target for one exported cube."""

    cube_id: str
    exported: ExportedCube
    target_path: Path
    existing_path: Optional[Path] = None
    previous_cube_id: str = ""
    existing_payload: Optional[Mapping[str, Any]] = None
    action_kind: str = "update"
    forked: bool = False
    source_revision_ref: str = ""
    source_version: str = ""
    source_definition_key: str = ""
    stale_save_mode: str = ""


@dataclass(frozen=True)
class CubeSaveCommitState:
    """Describe commit eligibility and outcome for one saved cube."""

    should_commit: bool
    action_kind: str
    previous_version: str
    resulting_version: str
    version_changed: bool = False
    change_scope: str = "none"
    previous_cube_id: str = ""
    commit_result: Optional[CubeCommitResult] = None
    commit_error: str = ""


@dataclass(frozen=True)
class FinalizedCubeSave:
    """Represent one persisted cube and its authoritative read model."""

    target: CubeSaveTarget
    document: CubeDocument
    artifact: Mapping[str, Any]
    commit_state: CubeSaveCommitState
    definition: Mapping[str, Any]


def extract_uuid_wrapper_classes(graph: Mapping[str, Any]) -> set[str]:
    """Return UUID wrapper class types referenced by the graph."""

    wrappers: set[str] = set()
    for node_id, node in graph.items():
        if node_id == "workflow" or not isinstance(node, Mapping):
            continue
        class_type = node.get("class_type")
        class_name = class_type.strip() if isinstance(class_type, str) else ""
        if class_name and _UUID_CLASS_RE.match(class_name):
            wrappers.add(class_name)
    return wrappers


def index_workflow_subgraphs(
    workflow: Mapping[str, Any],
) -> dict[str, Mapping[str, Any]]:
    """Index workflow subgraph definitions by id."""

    definitions = workflow.get("definitions")
    if not isinstance(definitions, Mapping):
        return {}
    subgraphs = definitions.get("subgraphs")
    if not isinstance(subgraphs, Sequence):
        return {}
    index: dict[str, Mapping[str, Any]] = {}
    for entry in subgraphs:
        if not isinstance(entry, Mapping):
            continue
        subgraph_id = entry.get("id")
        if isinstance(subgraph_id, str) and subgraph_id:
            index[subgraph_id] = dict(entry)
    return index


def subgraph_has_executable_body(definition: Mapping[str, Any]) -> bool:
    """Return whether a subgraph definition contains executable nodes."""

    nodes = definition.get("nodes")
    if not isinstance(nodes, Sequence):
        return False
    for node in nodes:
        if not isinstance(node, Mapping):
            continue
        node_type = node.get("type")
        if not isinstance(node_type, str):
            node_type = node.get("class_type")
        if isinstance(node_type, str) and node_type.strip():
            return True
    return False


def collect_subgraph_contract_violations(
    graph: Mapping[str, Any],
    workflow: Mapping[str, Any],
) -> dict[str, list[str]]:
    """Collect malformed subgraph wrapper definitions."""

    wrapper_ids = extract_uuid_wrapper_classes(graph)
    if not wrapper_ids:
        return {}
    indexed = index_workflow_subgraphs(workflow)
    missing = sorted(wrapper_ids - set(indexed.keys()))
    empty = sorted(
        wrapper_id
        for wrapper_id in wrapper_ids & set(indexed.keys())
        if not subgraph_has_executable_body(indexed[wrapper_id])
    )
    violations: dict[str, list[str]] = {}
    if missing:
        violations["missing_subgraphs"] = missing
    if empty:
        violations["empty_subgraph_bodies"] = empty
    label_violations = collect_subgraph_interface_label_violations(
        indexed, sorted(wrapper_ids & set(indexed.keys()))
    )
    violations.update(label_violations)
    return violations


def collect_subgraph_interface_label_violations(
    indexed: Mapping[str, Mapping[str, Any]], wrapper_ids: Sequence[str]
) -> dict[str, list[str]]:
    """Collect missing or duplicate public subgraph IO labels."""

    missing_labels: list[str] = []
    duplicate_labels: list[str] = []
    for wrapper_id in sorted(set(wrapper_ids)):
        definition = indexed.get(wrapper_id)
        if not isinstance(definition, Mapping):
            continue
        for direction in ("inputs", "outputs"):
            entries = definition.get(direction)
            if not isinstance(entries, Sequence) or isinstance(entries, (str, bytes)):
                continue
            labels: dict[str, str] = {}
            for index, entry in enumerate(entries):
                if not isinstance(entry, Mapping):
                    continue
                name = normalize_metadata_string(entry.get("name"))
                label = normalize_metadata_string(entry.get("label"))
                slot = name or f"#{index + 1}"
                if not name or not label:
                    missing_labels.append(f"{wrapper_id}.{direction}.{slot}")
                    continue
                previous = labels.get(label)
                if previous is not None:
                    duplicate_labels.append(
                        f"{wrapper_id}.{direction}.{label}: {previous}, {name}"
                    )
                labels[label] = name
    violations: dict[str, list[str]] = {}
    if missing_labels:
        violations["missing_subgraph_labels"] = missing_labels
    if duplicate_labels:
        violations["duplicate_subgraph_labels"] = duplicate_labels
    return violations


def collect_subgraph_node_class_types(workflow: Mapping[str, Any]) -> set[str]:
    """Collect concrete node class types declared in workflow subgraphs."""

    required: set[str] = set()
    for definition in index_workflow_subgraphs(workflow).values():
        nodes = definition.get("nodes")
        if not isinstance(nodes, Sequence):
            continue
        for node in nodes:
            if not isinstance(node, Mapping):
                continue
            class_type = node.get("type")
            normalized = class_type.strip() if isinstance(class_type, str) else ""
            if normalized and not _UUID_CLASS_RE.match(normalized):
                required.add(normalized)
    return required


def collect_required_node_class_types(
    graph: Mapping[str, Any],
    workflow: Mapping[str, Any],
) -> set[str]:
    """Collect all runtime class types needed for export validation."""

    required = collect_subgraph_node_class_types(workflow)
    for node_id, node in graph.items():
        if node_id == "workflow" or not isinstance(node, Mapping):
            continue
        class_type = node.get("class_type")
        normalized = class_type.strip() if isinstance(class_type, str) else ""
        if normalized and not _UUID_CLASS_RE.match(normalized):
            required.add(normalized)
    return required


def collect_selected_cube_ids(
    analysis: CubeAnalysis, cube_ids: Collection[str]
) -> list[str]:
    """Return requested cube ids that exist in the analyzed graph."""

    requested_ids = {cube_id for cube_id in cube_ids if cube_id}
    return sorted(cube_id for cube_id in requested_ids if cube_id in analysis.cubes)


def collect_selected_cube_wrapper_classes(
    analysis: CubeAnalysis, cube_ids: Collection[str]
) -> set[str]:
    """Collect UUID wrapper class types used by the selected cube set."""

    wrapper_ids: set[str] = set()
    for cube_id in collect_selected_cube_ids(analysis, cube_ids):
        cube = analysis.cubes[cube_id]
        for node_id in cube.subgraph_nodes:
            node = analysis.graph.nodes.get(node_id)
            if not node:
                continue
            class_name = node.class_type.strip()
            if class_name and _UUID_CLASS_RE.match(class_name):
                wrapper_ids.add(class_name)
    return wrapper_ids


def collect_selected_cube_subgraph_node_class_types(
    workflow: Mapping[str, Any], wrapper_ids: Sequence[str]
) -> set[str]:
    """Collect concrete class types from selected wrapper subgraph definitions."""

    indexed = index_workflow_subgraphs(workflow)
    required: set[str] = set()
    for wrapper_id in sorted(set(wrapper_ids)):
        definition = indexed.get(wrapper_id)
        if not isinstance(definition, Mapping):
            continue
        nodes = definition.get("nodes")
        if not isinstance(nodes, Sequence):
            continue
        for node in nodes:
            if not isinstance(node, Mapping):
                continue
            class_type = node.get("type")
            if not isinstance(class_type, str):
                class_type = node.get("class_type")
            normalized = class_type.strip() if isinstance(class_type, str) else ""
            if normalized and not _UUID_CLASS_RE.match(normalized):
                required.add(normalized)
    return required


def collect_selected_cube_required_node_class_types(
    analysis: CubeAnalysis,
    workflow: Mapping[str, Any],
    cube_ids: Collection[str],
) -> set[str]:
    """Collect runtime class types required by the selected cube set only."""

    required: set[str] = set()
    wrapper_ids = collect_selected_cube_wrapper_classes(analysis, cube_ids)
    for cube_id in collect_selected_cube_ids(analysis, cube_ids):
        cube = analysis.cubes[cube_id]
        for node_id in cube.subgraph_nodes:
            node = analysis.graph.nodes.get(node_id)
            if not node:
                continue
            normalized = node.class_type.strip()
            if normalized and not _UUID_CLASS_RE.match(normalized):
                required.add(normalized)
    required.update(
        collect_selected_cube_subgraph_node_class_types(workflow, sorted(wrapper_ids))
    )
    return required


def collect_selected_cube_subgraph_contract_violations(
    analysis: CubeAnalysis,
    workflow: Mapping[str, Any],
    cube_ids: Collection[str],
) -> dict[str, list[str]]:
    """Collect malformed wrapper definitions for the selected cube set."""

    wrapper_ids = collect_selected_cube_wrapper_classes(analysis, cube_ids)
    if not wrapper_ids:
        return {}
    indexed = index_workflow_subgraphs(workflow)
    missing = sorted(wrapper_ids - set(indexed.keys()))
    empty = sorted(
        wrapper_id
        for wrapper_id in wrapper_ids & set(indexed.keys())
        if not subgraph_has_executable_body(indexed[wrapper_id])
    )
    violations: dict[str, list[str]] = {}
    if missing:
        violations["missing_subgraphs"] = missing
    if empty:
        violations["empty_subgraph_bodies"] = empty
    label_violations = collect_subgraph_interface_label_violations(
        indexed, sorted(wrapper_ids & set(indexed.keys()))
    )
    violations.update(label_violations)
    return violations


def collect_missing_node_class_types(
    class_types: Collection[str],
    node_class_mappings: Mapping[str, Any],
) -> list[str]:
    """Return sorted class types missing from the active Comfy registry."""

    return sorted(
        class_type
        for class_type in set(class_types)
        if class_type not in node_class_mappings
    )


class CubeExportService:
    """Own save-many request orchestration and export validation."""

    def __init__(
        self,
        library_service: CubeLibraryService,
        *,
        export_cubes: Callable[..., list[ExportedCube]],
        write_cube: Callable[..., Mapping[str, Any]],
        write_cubes: Callable[..., Sequence[Mapping[str, Any]]],
        write_cube_to_path: Callable[..., Mapping[str, Any]],
        write_cubes_to_paths: Callable[..., Sequence[Mapping[str, Any]]],
        suggest_version: Callable[[Mapping[str, Any], Mapping[str, Any]], Any],
        node_class_mappings_provider: NodeClassMappingsProvider,
        finalized_definition_provider: FinalizedDefinitionProvider,
        local_flavor_service: Optional[LocalFlavorService] = None,
    ) -> None:
        """Initialize the export service."""

        self.library_service = library_service
        self.export_cubes = export_cubes
        self.write_cube = write_cube
        self.write_cubes = write_cubes
        self.write_cube_to_path = write_cube_to_path
        self.write_cubes_to_paths = write_cubes_to_paths
        self.suggest_version = suggest_version
        self.node_class_mappings_provider = node_class_mappings_provider
        self.finalized_definition_provider = finalized_definition_provider
        self.local_flavor_service = local_flavor_service

    def _resolve_node_class_mappings(self) -> Mapping[str, Any]:
        """Resolve the current Comfy node registry for this export attempt."""

        mappings = self.node_class_mappings_provider()
        return mappings

    def save_many(
        self,
        *,
        graph: Mapping[str, Any],
        workflow: Mapping[str, Any],
        workflow_version: Optional[int],
        actor: Mapping[str, str],
        cube_entries: Mapping[str, Mapping[str, Any]],
    ) -> dict[str, Any]:
        """Export all requested cubes while preserving current response shapes."""

        if not cube_entries:
            return {"saved": []}

        try:
            default_alias_lookup = self.library_service.build_default_alias_lookup(
                cube_entries.keys()
            )
            analysis = analyze_cubes(
                graph,
                workflow=workflow,
                default_alias_lookup=default_alias_lookup,
            )
            subgraph_violations = collect_selected_cube_subgraph_contract_violations(
                analysis,
                workflow,
                cube_entries.keys(),
            )
            if subgraph_violations:
                raise BackendError(
                    "Workflow definitions.subgraphs must include executable bodies and labeled public IO for all UUID wrapper nodes.",
                    status=400,
                    details=subgraph_violations,
                )

            required_class_types = collect_selected_cube_required_node_class_types(
                analysis,
                workflow,
                cube_entries.keys(),
            )
            missing_class_types = collect_missing_node_class_types(
                sorted(required_class_types),
                self._resolve_node_class_mappings(),
            )
            if missing_class_types:
                raise BackendError(
                    "Cannot export cube(s): required node class definitions are missing from the active Comfy registry.",
                    status=400,
                    details={"missing_class_types": missing_class_types},
                )

            cubes = self.export_cubes(
                graph,
                workflow=workflow,
                workflow_version=workflow_version,
                default_alias_lookup=default_alias_lookup,
                cube_ids=list(cube_entries.keys()),
            )
            if not cubes:
                return {"saved": []}
            return self._finalize_exports(cubes, cube_entries, actor)
        except CubeValidationError as exc:
            raise BackendError(
                exc.message,
                status=400,
                details=exc.details or None,
                extra={"violations": exc.violations},
            ) from exc
        except ValueError as exc:
            raise BackendError(str(exc), status=400) from exc
        except FileExistsError as exc:
            raise BackendError(str(exc), status=409) from exc
        except BackendError:
            raise
        except Exception as exc:  # pragma: no cover - defensive
            _logger.exception(
                "SugarCubes export failed while saving %d cube(s)",
                len(cube_entries),
            )
            raise BackendError(
                "Export failed",
                status=500,
                details={"reason": str(exc)},
            ) from exc

    def save_implementation(
        self,
        *,
        graph: Mapping[str, Any],
        workflow: Mapping[str, Any],
        workflow_version: Optional[int],
        actor: Mapping[str, str],
        cube_entries: Mapping[str, Mapping[str, Any]],
    ) -> dict[str, Any]:
        """Persist implementation and cosmetic changes through the explicit route."""

        return self.save_many(
            graph=graph,
            workflow=workflow,
            workflow_version=workflow_version,
            actor=actor,
            cube_entries=cube_entries,
        )

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
        commit_state = self._build_commit_state(
            save_target=save_target,
            previous_payload=payload,
        )
        finalized_commit_state = self._finalize_commit(
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
                **self._build_finalized_save_response(finalized),
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

    def _finalize_exports(
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
            if entry.get("description_set"):
                exported.cube["description"] = normalize_metadata_string(
                    entry.get("description")
                )
            metadata = exported.cube.get("metadata")
            if not isinstance(metadata, dict):
                metadata = {}
                exported.cube["metadata"] = metadata
            entry_metadata = (
                entry.get("metadata")
                if isinstance(entry.get("metadata"), Mapping)
                else {}
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

            sanitize_authored_defaults_payload(exported.cube)
            save_target = self._build_author_save_target(
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
            if existing_payload is None:
                commit_states[cube_id] = self._build_commit_state(
                    save_target=save_target,
                    previous_payload=None,
                )
                continue
            self._preserve_authored_flavors_on_existing_implementation_save(
                existing_payload=existing_payload,
                exported=exported,
                preserve_description=not bool(entry.get("description_set")),
            )
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
            commit_states[cube_id] = self._build_commit_state(
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
                ) or self._build_commit_state(
                    save_target=save_target,
                    previous_payload=save_target.existing_payload,
                )
                finalized_commit_state = self._finalize_commit(
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
                saved.append(self._build_finalized_save_response(finalized))

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

    def _build_finalized_save_response(
        self, finalized: FinalizedCubeSave
    ) -> dict[str, Any]:
        """Project one authoritative persisted save into the HTTP response."""

        commit_result = finalized.commit_state.commit_result
        return {
            **finalized.artifact,
            "cube_id": finalized.target.cube_id,
            "forked": False,
            "committed": bool(commit_result),
            "commit_sha": commit_result.commit_sha if commit_result else "",
            "commit_short_sha": (
                commit_result.commit_short_sha if commit_result else ""
            ),
            "commit_message": commit_result.commit_message if commit_result else "",
            "commit_error": finalized.commit_state.commit_error,
            "version": normalize_metadata_string(finalized.document.version),
            "definition": deepcopy(dict(finalized.definition)),
        }

    def _preserve_authored_flavors_on_existing_implementation_save(
        self,
        *,
        existing_payload: Mapping[str, Any],
        exported: ExportedCube,
        preserve_description: bool = True,
    ) -> None:
        """Keep catalog metadata and authored presets while replacing implementation data."""

        try:
            existing_document = CubeDocument.from_dict(existing_payload)
            exported_document = CubeDocument.from_dict(exported.cube)
            exported_document = self._preserve_catalog_metadata_on_implementation_save(
                existing_document=existing_document,
                exported_document=exported_document,
                exported_default_alias=exported.default_alias,
                preserve_description=preserve_description,
            )
            merged_document = preserve_authored_flavors_for_implementation_save(
                existing_document,
                exported_document,
            )
            merged_document = sanitize_authored_defaults_document(merged_document)
        except CubeSchemaError as exc:
            raise BackendError(str(exc), status=400) from exc

        exported.cube.clear()
        exported.cube.update(merged_document.to_dict())

    def _preserve_catalog_metadata_on_implementation_save(
        self,
        *,
        existing_document: CubeDocument,
        exported_document: CubeDocument,
        exported_default_alias: str,
        preserve_description: bool,
    ) -> CubeDocument:
        """Preserve existing catalog fields that the implementation export omitted."""

        payload = exported_document.to_dict()
        existing_description = normalize_metadata_string(existing_document.description)
        exported_description = normalize_metadata_string(exported_document.description)
        if (
            preserve_description
            and existing_description
            and exported_description.startswith("Auto-converted cube for ")
        ):
            payload["description"] = existing_description

        metadata = {
            **existing_document.metadata,
            **exported_document.metadata,
        }
        default_alias = normalize_metadata_string(metadata.get("default_alias"))
        normalized_exported_default_alias = normalize_metadata_string(
            exported_default_alias
        )
        if not default_alias and normalized_exported_default_alias:
            metadata["default_alias"] = normalized_exported_default_alias
        metadata.pop("author", None)
        if metadata:
            payload["metadata"] = metadata
        return CubeDocument.from_dict(payload)

    def _build_author_save_target(
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

    def _build_commit_state(
        self,
        *,
        save_target: CubeSaveTarget,
        previous_payload: Optional[Mapping[str, Any]],
    ) -> CubeSaveCommitState:
        """Build commit eligibility metadata from the previous and next payloads."""

        previous_version = normalize_metadata_string(
            previous_payload.get("version") if previous_payload else ""
        )
        resulting_version = normalize_metadata_string(
            save_target.exported.cube.get("version")
        )
        version_changed = (
            bool(resulting_version) and previous_version != resulting_version
        )
        change_scope = self._classify_saved_change_scope(
            previous_payload=previous_payload,
            next_payload=save_target.exported.cube,
            version_changed=version_changed,
        )
        return CubeSaveCommitState(
            should_commit=False,
            action_kind=save_target.action_kind,
            previous_version=previous_version,
            resulting_version=resulting_version,
            version_changed=version_changed,
            change_scope=change_scope,
            previous_cube_id=save_target.previous_cube_id,
        )

    def _classify_saved_change_scope(
        self,
        *,
        previous_payload: Optional[Mapping[str, Any]],
        next_payload: Mapping[str, Any],
        version_changed: bool,
    ) -> str:
        """Return a commit-message scope for one saved cube change."""

        if version_changed:
            return "version"
        if previous_payload is None:
            return "content"
        previous_without_layout = self._without_layout_fields(previous_payload)
        next_without_layout = self._without_layout_fields(next_payload)
        if previous_without_layout == next_without_layout:
            return "layout"
        return "content"

    def _without_layout_fields(self, payload: Mapping[str, Any]) -> dict[str, Any]:
        """Return a payload copy with persisted layout-only fields removed."""

        stripped = deepcopy(dict(payload))
        stripped.pop("layout", None)
        implementation = stripped.get("implementation")
        if isinstance(implementation, dict):
            implementation.pop("layout", None)
        metadata = stripped.get("metadata")
        if isinstance(metadata, dict):
            metadata.pop("layout", None)
        return stripped

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

    def _finalize_commit(
        self,
        *,
        save_target: CubeSaveTarget,
        commit_state: CubeSaveCommitState,
    ) -> CubeSaveCommitState:
        """Commit one saved cube file when the persisted content changed."""

        try:
            git_context = resolve_cube_git_context(
                self.library_service.tracked_repo_service,
                save_target.cube_id,
            )
            if not self.library_service.tracked_repo_service.has_file_changes(
                repo_root=git_context.repo_root,
                repo_relative_path=git_context.repo_relative_path,
            ):
                return CubeSaveCommitState(
                    should_commit=False,
                    action_kind=commit_state.action_kind,
                    previous_version=commit_state.previous_version,
                    resulting_version=commit_state.resulting_version,
                    version_changed=commit_state.version_changed,
                    change_scope="none",
                    previous_cube_id=commit_state.previous_cube_id,
                )
            commit_result = self.library_service.tracked_repo_service.commit_file(
                repo_root=git_context.repo_root,
                repo_relative_path=git_context.repo_relative_path,
                commit_message=self._build_commit_message(
                    save_target=save_target,
                    commit_state=commit_state,
                    git_context=git_context,
                ),
            )
            return CubeSaveCommitState(
                should_commit=True,
                action_kind=commit_state.action_kind,
                previous_version=commit_state.previous_version,
                resulting_version=commit_state.resulting_version,
                version_changed=commit_state.version_changed,
                change_scope=commit_state.change_scope,
                previous_cube_id=commit_state.previous_cube_id,
                commit_result=commit_result,
            )
        except BackendError as exc:
            _logger.warning(
                "SugarCubes: save wrote cube '%s' but git commit failed",
                save_target.cube_id,
                exc_info=exc,
            )
            return CubeSaveCommitState(
                should_commit=True,
                action_kind=commit_state.action_kind,
                previous_version=commit_state.previous_version,
                resulting_version=commit_state.resulting_version,
                version_changed=commit_state.version_changed,
                change_scope=commit_state.change_scope,
                previous_cube_id=commit_state.previous_cube_id,
                commit_error=exc.message,
            )

    def _build_commit_message(
        self,
        *,
        save_target: CubeSaveTarget,
        commit_state: CubeSaveCommitState,
        git_context: CubeGitContext,
    ) -> str:
        """Build the concise commit subject for one cube save."""

        target_name = Path(git_context.repo_relative_path).name
        version = commit_state.resulting_version
        version_suffix = (
            f" v{version}" if commit_state.version_changed and version else ""
        )
        scope_suffix = (
            f" {commit_state.change_scope}"
            if not version_suffix and commit_state.change_scope in {"layout", "content"}
            else ""
        )
        if commit_state.action_kind == "rename" and commit_state.previous_cube_id:
            previous_name = Path(
                commit_state.previous_cube_id.split("/", maxsplit=3)[-1]
            ).name
            return (
                f"rename {previous_name} to {target_name}{version_suffix}{scope_suffix}"
            )
        if commit_state.action_kind == "fork":
            previous_name = (
                Path(commit_state.previous_cube_id.split("/", maxsplit=3)[-1]).name
                if commit_state.previous_cube_id
                else target_name
            )
            return (
                f"fork {previous_name} as {save_target.cube_id}"
                f"{version_suffix}{scope_suffix}"
            )
        return f"{commit_state.action_kind} {target_name}{version_suffix}{scope_suffix}"

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
