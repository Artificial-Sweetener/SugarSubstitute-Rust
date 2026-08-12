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
"""Coordinate Cube export preparation and persistence owners."""

from __future__ import annotations

import logging
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import Any, Optional

from ...cube_model import sanitize_authored_defaults_payload
from ...exporter import CubeValidationError, ExportedCube
from ...exporter.graph import analyze_cubes
from ...exporter.native_subgraph_adapter import project_native_cube_exports
from ..responses import BackendError
from .cube_authored_flavor_save_service import CubeAuthoredFlavorSaveService
from .cube_export_graph_contract import (
    collect_missing_node_class_types,
    collect_selected_cube_required_node_class_types,
    collect_selected_cube_subgraph_contract_violations,
)
from .cube_export_persistence import CubeExportPersistence
from .cube_implementation_defaults_service import CubeImplementationDefaultsService
from .cube_library_service import CubeLibraryService
from .cube_metadata import normalize_metadata_string
from .cube_save_commit_finalizer import CubeSaveCommitFinalizer
from .cube_save_target_planner import CubeSaveTargetPlanner
from .local_flavor_service import LocalFlavorService

_logger = logging.getLogger(__name__)
NodeClassMappingsProvider = Callable[[], Mapping[str, Any]]
FinalizedDefinitionProvider = Callable[
    [Path, str, Mapping[str, Any]], Mapping[str, Any]
]


class CubeExportService:
    """Coordinate export validation, preview, and persistence collaborators."""

    def __init__(
        self,
        library_service: CubeLibraryService,
        *,
        export_cubes: Callable[..., list[ExportedCube]],
        write_cube_to_path: Callable[..., Mapping[str, Any]],
        write_cubes_to_paths: Callable[..., Sequence[Mapping[str, Any]]],
        suggest_version: Callable[[Mapping[str, Any], Mapping[str, Any]], Any],
        node_class_mappings_provider: NodeClassMappingsProvider,
        finalized_definition_provider: FinalizedDefinitionProvider,
        local_flavor_service: Optional[LocalFlavorService] = None,
    ) -> None:
        """Compose export owners around stable library and writer boundaries."""

        self.library_service = library_service
        self.export_cubes = export_cubes
        self.node_class_mappings_provider = node_class_mappings_provider
        self.implementation_defaults = CubeImplementationDefaultsService()
        self.targets = CubeSaveTargetPlanner(library_service)
        self.commits = CubeSaveCommitFinalizer(library_service)
        self.persistence = CubeExportPersistence(
            library_service,
            write_cubes_to_paths=write_cubes_to_paths,
            suggest_version=suggest_version,
            finalized_definition_provider=finalized_definition_provider,
            implementation_defaults=self.implementation_defaults,
            targets=self.targets,
            commits=self.commits,
        )
        self.authored_flavors = CubeAuthoredFlavorSaveService(
            library_service,
            write_cube_to_path=write_cube_to_path,
            suggest_version=suggest_version,
            finalized_definition_provider=finalized_definition_provider,
            commits=self.commits,
            local_flavor_service=local_flavor_service,
        )

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
            cubes = self._prepare_exports(
                graph=graph,
                workflow=workflow,
                workflow_version=workflow_version,
                cube_entries=cube_entries,
            )
            if not cubes:
                return {"saved": []}
            return self.persistence.finalize_exports(cubes, cube_entries, actor)
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
                "SugarCubes export failed while saving %d cube(s)", len(cube_entries)
            )
            raise BackendError(
                "Export failed", status=500, details={"reason": str(exc)}
            ) from exc

    def preview_implementation(
        self,
        *,
        graph: Mapping[str, Any],
        workflow: Mapping[str, Any],
        workflow_version: Optional[int],
        actor: Mapping[str, str],
        cube_entries: Mapping[str, Mapping[str, Any]],
    ) -> dict[str, Any]:
        """Return authoritative default-change plans without writing artifacts."""

        try:
            cubes = self._prepare_exports(
                graph=graph,
                workflow=workflow,
                workflow_version=workflow_version,
                cube_entries=cube_entries,
            )
            exported_lookup = {
                cube_id: exported
                for exported in cubes
                if (cube_id := normalize_metadata_string(exported.cube.get("cube_id")))
            }
            reviews: list[dict[str, Any]] = []
            for cube_id, entry in cube_entries.items():
                exported = exported_lookup.get(cube_id)
                if exported is None:
                    raise BackendError(
                        "Cube id missing from implementation preview",
                        status=400,
                        details={"cube_id": cube_id},
                    )
                self.persistence.apply_entry_metadata(exported, cube_id, entry, actor)
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
                    source_version=normalize_metadata_string(
                        entry.get("source_version")
                    ),
                    source_definition_key=normalize_metadata_string(
                        entry.get("source_definition_key")
                    ),
                    stale_save_mode=normalize_metadata_string(
                        entry.get("stale_save_mode")
                    ),
                )
                reviews.append(
                    self.implementation_defaults.review(
                        existing_payload=save_target.existing_payload,
                        exported=exported,
                        preserve_description=not bool(entry.get("description_set")),
                    )
                )
            return {"reviews": reviews}
        except CubeValidationError as exc:
            raise BackendError(
                exc.message,
                status=400,
                details=exc.details or None,
                extra={"violations": exc.violations},
            ) from exc
        except ValueError as exc:
            raise BackendError(str(exc), status=400) from exc

    def _prepare_exports(
        self,
        *,
        graph: Mapping[str, Any],
        workflow: Mapping[str, Any],
        workflow_version: Optional[int],
        cube_entries: Mapping[str, Mapping[str, Any]],
    ) -> list[ExportedCube]:
        """Validate and project one immutable set of implementation candidates."""

        default_alias_lookup = self.library_service.build_default_alias_lookup(
            cube_entries.keys()
        )
        projected_graph, projected_workflow = project_native_cube_exports(
            graph, workflow, cube_entries
        )
        analysis = analyze_cubes(
            projected_graph,
            workflow=projected_workflow,
            default_alias_lookup=default_alias_lookup,
        )
        subgraph_violations = collect_selected_cube_subgraph_contract_violations(
            analysis, projected_workflow, cube_entries.keys()
        )
        if subgraph_violations:
            raise BackendError(
                "Workflow definitions.subgraphs must include executable bodies and labeled public IO for all UUID wrapper nodes.",
                status=400,
                details=subgraph_violations,
            )
        required_class_types = collect_selected_cube_required_node_class_types(
            analysis, projected_workflow, cube_entries.keys()
        )
        missing_class_types = collect_missing_node_class_types(
            sorted(required_class_types), self.node_class_mappings_provider()
        )
        if missing_class_types:
            raise BackendError(
                "Cannot export cube(s): required node class definitions are missing from the active Comfy registry.",
                status=400,
                details={"missing_class_types": missing_class_types},
            )
        return self.export_cubes(
            projected_graph,
            workflow=projected_workflow,
            workflow_version=workflow_version,
            default_alias_lookup=default_alias_lookup,
            cube_ids=list(cube_entries.keys()),
        )

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

        return self.authored_flavors.save_authored_flavor(
            cube_id=cube_id,
            values=values,
            flavor_id=flavor_id,
            flavor_name=flavor_name,
        )
