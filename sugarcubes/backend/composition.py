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
"""Compose the SugarCubes backend service graph."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

from ..authoring import (
    LegacyWorkflowAuthoringService,
    NativeWorkflowNormalizer,
    SugarScriptWorkflowAuthoringService,
)
from ..exporter import (
    export as export_cubes,
    write_cube_to_path,
    write_cubes_to_paths,
)
from ..exporter.versioning import suggest_version
from ..execution import CubeExecutionCoordinator
from ..execution import LiveNodeDefaultReconciler
from ..workflow_analysis import CubeGraphAnalysisService
from ..workflow_mutation import CubeGraphMutationService
from ..exporter.definition_snapshot import resolve_definition_via_nodes
from ..execution.ports import ComfyExecutionPort
from ..importer import load_cube as load_cube_artifact
from ..importer import load_materialized_cube_document
from ..importer import prepare_import as prepare_cube_import
from ..language import SugarScriptLanguageService
from ..library import (
    CubeForkService,
    CubeLibraryClassService,
    CubeSourceSyncService,
    CapturedCubeRepository,
)
from ..payloads import retarget_cube_payload
from .comfy_node_registry import resolve_active_comfy_node_class_mappings
from .services import (
    CubeArtifactRepository,
    CubeDependencyService,
    CubeExportService,
    CubeHistoryService,
    CubeIdentityRedirectService,
    CubeLibraryService,
    CubeLoadService,
    CubePickerCatalogService,
    CubeMetadataService,
    CubePromotionService,
    CubeRevisionService,
    IdentityPolicyService,
    LocalFlavorService,
    NativeCubeImportPreparerAdapter,
    OwnershipPolicyService,
    TrackedRepoService,
    SugarScriptCatalogResolver,
    WorkflowCatalogArtifactProvider,
    WorkflowForkRepository,
    WorkflowSourceSyncPort,
)


@dataclass(frozen=True)
class BackendServices:
    """Bundle the services shared by host routes and public integrations."""

    library: CubeLibraryService
    picker_catalog: CubePickerCatalogService
    tracked_repos: TrackedRepoService
    identity: IdentityPolicyService
    ownership: OwnershipPolicyService
    metadata: CubeMetadataService
    promotion: CubePromotionService
    redirects: CubeIdentityRedirectService
    exporter: CubeExportService
    loader: CubeLoadService
    revisions: CubeRevisionService
    local_flavors: LocalFlavorService
    dependencies: CubeDependencyService
    workflow_library: CubeLibraryClassService
    workflow_forks: CubeForkService
    workflow_source_sync: CubeSourceSyncService
    sugarscript_authoring: SugarScriptWorkflowAuthoringService
    legacy_workflow_authoring: LegacyWorkflowAuthoringService
    execution: CubeExecutionCoordinator = field(
        default_factory=CubeExecutionCoordinator
    )
    workflow_analysis: CubeGraphAnalysisService = field(
        default_factory=CubeGraphAnalysisService
    )
    workflow_mutation: CubeGraphMutationService = field(
        default_factory=lambda: CubeGraphMutationService(CubeGraphAnalysisService())
    )


def build_backend_services(
    extension_root: Path,
    *,
    workspace_path: Path | None = None,
    custom_nodes_root: Path | None = None,
    execution_port: ComfyExecutionPort | None = None,
) -> BackendServices:
    """Build the repository-standard backend service graph."""

    identity = IdentityPolicyService(extension_root)
    tracked_repos = TrackedRepoService(
        extension_root,
        protected_owner_provider=lambda: identity.get_policy().claimed_github_owner,
    )
    local_flavors = LocalFlavorService(tracked_repos)
    artifacts = CubeArtifactRepository(tracked_repos)
    history = CubeHistoryService(tracked_repos)
    redirects = CubeIdentityRedirectService(tracked_repos)
    ownership = OwnershipPolicyService(
        tracked_repo_service=tracked_repos,
        identity_policy_service=identity,
    )
    library = CubeLibraryService(
        extension_root,
        load_cube_artifact=load_cube_artifact,
        prepare_cube_import=prepare_cube_import,
        tracked_repo_service=tracked_repos,
        ownership_policy_service=ownership,
        registry_factory=None,
    )
    workflow_catalog = WorkflowCatalogArtifactProvider(library.catalog_listing)
    workflow_library = CubeLibraryClassService(
        captured=CapturedCubeRepository(
            tracked_repos.data_root() / "captured_cubes",
            legacy_root=tracked_repos.data_root() / "wild_cube_stable",
        ),
        catalog_artifacts=workflow_catalog.list_artifacts,
    )
    workflow_forks = CubeForkService(
        WorkflowForkRepository(
            artifacts=artifacts,
            ownership=ownership,
            library=library,
        )
    )
    workflow_source_sync = CubeSourceSyncService(
        WorkflowSourceSyncPort(
            tracked_repos,
            catalog_artifacts=workflow_catalog.list_artifacts,
        )
    )
    workflow_resolver = SugarScriptCatalogResolver(
        library=library,
        redirects=redirects,
        load_cube=load_cube_artifact,
    )
    workflow_preparer = NativeCubeImportPreparerAdapter(
        load_document=lambda document: load_materialized_cube_document(document),
        prepare_import=lambda loaded: prepare_cube_import(loaded),
        resolve_live_definition=resolve_definition_via_nodes,
    )
    sugarscript_authoring = SugarScriptWorkflowAuthoringService(
        language=SugarScriptLanguageService(),
        resolver=workflow_resolver,
        preparer=workflow_preparer,
    )
    legacy_workflow_authoring = LegacyWorkflowAuthoringService(
        resolver=workflow_resolver,
        preparer=workflow_preparer,
    )
    workflow_normalizer = NativeWorkflowNormalizer(workflow_resolver)
    metadata = CubeMetadataService(
        library,
        retarget_cube_payload=retarget_cube_payload,
        artifacts=artifacts,
        history=history,
        local_flavors=local_flavors,
    )
    promotion = CubePromotionService(
        artifacts=artifacts,
        history=history,
        redirects=redirects,
        local_flavors=local_flavors,
        ownership=ownership,
        library=library,
        retarget_cube_payload=retarget_cube_payload,
    )
    loader = CubeLoadService(
        library,
        load_cube_artifact=load_cube_artifact,
        prepare_cube_import=prepare_cube_import,
        redirect_service=redirects,
    )
    picker_catalog = CubePickerCatalogService(
        list_catalog=library.list_library_catalog,
        load_cube=lambda cube_id: loader.load_cube(
            cube_id=cube_id,
            version_pin="",
            drop_origin=(0.0, 0.0),
        ),
    )
    exporter = CubeExportService(
        library,
        export_cubes=export_cubes,
        write_cube_to_path=write_cube_to_path,
        write_cubes_to_paths=write_cubes_to_paths,
        suggest_version=suggest_version,
        node_class_mappings_provider=lambda: resolve_active_comfy_node_class_mappings(
            extension_root
        ),
        finalized_definition_provider=lambda path, cube_id, _payload: (
            loader.load_cube_path(
                cube_path=path,
                cube_id=cube_id,
            )
        ),
        local_flavor_service=local_flavors,
    )
    revisions = CubeRevisionService(
        library,
        tracked_repos,
        load_cube_artifact=load_cube_artifact,
        prepare_cube_import=prepare_cube_import,
        redirect_service=redirects,
    )
    dependencies = CubeDependencyService(
        library_service=library,
        tracked_repo_service=tracked_repos,
        workspace_path=workspace_path or extension_root.parent.parent,
        custom_nodes_root=custom_nodes_root or extension_root.parent,
    )
    workflow_analysis = CubeGraphAnalysisService(workflow_normalizer)
    return BackendServices(
        library=library,
        picker_catalog=picker_catalog,
        tracked_repos=tracked_repos,
        identity=identity,
        ownership=ownership,
        metadata=metadata,
        promotion=promotion,
        redirects=redirects,
        exporter=exporter,
        loader=loader,
        revisions=revisions,
        local_flavors=local_flavors,
        dependencies=dependencies,
        workflow_library=workflow_library,
        workflow_forks=workflow_forks,
        workflow_source_sync=workflow_source_sync,
        sugarscript_authoring=sugarscript_authoring,
        legacy_workflow_authoring=legacy_workflow_authoring,
        execution=CubeExecutionCoordinator(
            comfy=execution_port,
            live_defaults=LiveNodeDefaultReconciler(resolve_definition_via_nodes),
            workflow_normalizer=workflow_normalizer,
        ),
        workflow_analysis=workflow_analysis,
        workflow_mutation=CubeGraphMutationService(workflow_analysis),
    )
