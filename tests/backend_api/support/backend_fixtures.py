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
"""Provide typed backend route and service fixtures to backend-owned tests."""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast

import pytest

from sugarcubes.backend.composition import BackendServices
from sugarcubes.authoring import (
    LegacyWorkflowAuthoringService,
    SugarScriptWorkflowAuthoringService,
)
from sugarcubes.backend.services import (
    TrackedRepoPreflightResult,
    WorkflowCatalogArtifactProvider,
    WorkflowForkRepository,
    WorkflowSourceSyncPort,
)
from sugarcubes.library import (
    CubeForkService,
    CubeLibraryClassService,
    CubeSourceSyncService,
    CapturedCubeRepository,
)
from sugarcubes.importer import LoadedCube

from .typing_support import BackendServicesFactory
from tests.support.command_repository import CommandRepository

ROOT = Path(__file__).resolve().parents[3]

collect_ignore = [str(ROOT / "__init__.py")]


class FakeRequest:
    """Minimal aiohttp-like request object for route handler tests."""

    def __init__(
        self,
        *,
        body: object = None,
        query: dict[str, str] | None = None,
        json_error: BaseException | None = None,
    ) -> None:
        self._body = body
        self.query = query or {}
        self._json_error = json_error

    async def json(self) -> object:
        if self._json_error is not None:
            raise self._json_error
        return self._body


def decode_json_response(response: Any) -> Any:
    """Decode an aiohttp JSON response body for assertions."""

    return json.loads(response.body.decode("utf-8"))


class AllowingPreflightService:
    """Allow backend tests to avoid live GitHub preflight calls by default."""

    def inspect_repo(
        self, *, owner: str, repo: str, branch: str
    ) -> TrackedRepoPreflightResult:
        """Return a successful preflight result for one test repo."""

        return TrackedRepoPreflightResult(
            owner=owner,
            repo=repo,
            branch=branch,
            contains_cubes=True,
            cube_count=1,
            cube_paths=("demo.cube",),
        )

    def require_cubes(
        self, *, owner: str, repo: str, branch: str
    ) -> TrackedRepoPreflightResult:
        """Return a successful preflight result for one test repo."""

        return self.inspect_repo(owner=owner, repo=repo, branch=branch)


def ensure_tracked_repo(
    services: BackendServices,
    *,
    owner: str = "Artificial-Sweetener",
    repo: str = "Base-Cubes",
    branch: str = "main",
    enabled: bool = True,
    default_base_repo: bool = True,
) -> Path:
    """Create one tracked repo entry for backend tests and return its checkout path."""

    try:
        from sugarcubes.backend.responses import BackendError

        services.tracked_repos.get_repo(owner, repo)
    except BackendError as exc:
        if exc.status != 404:
            raise
        services.tracked_repos.add_repo(
            owner=owner,
            repo=repo,
            branch=branch,
            enabled=enabled,
            default_base_repo=default_base_repo,
        )
    checkout = services.tracked_repos.checkout_path(owner, repo)
    checkout.mkdir(parents=True, exist_ok=True)
    return checkout


def claim_github_owner(
    services: BackendServices,
    *,
    owner: str,
    allow_system_owner_claim: bool = False,
) -> None:
    """Persist one claimed GitHub owner for backend ownership tests."""

    if allow_system_owner_claim:
        (services.identity.extension_root / ".env").write_text(
            "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
            encoding="utf-8",
        )
    services.ownership.update_identity_policy(claimed_github_owner=owner)


@pytest.fixture
def backend_services_factory() -> BackendServicesFactory:
    """Build isolated backend services for route and service tests."""

    from sugarcubes.backend.services import (
        CubeArtifactRepository,
        CubeExportService,
        CubeDependencyService,
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
        OwnershipPolicyService,
        TrackedRepoService,
        NativeCubeImportPreparerAdapter,
        SugarScriptCatalogResolver,
    )
    from sugarcubes.importer import load_materialized_cube_document, prepare_import
    from sugarcubes.language import SugarScriptLanguageService

    def factory(
        tmp_path: Path,
        *,
        load_cube_artifact: Any = None,
        prepare_cube_import: Any = None,
        export_cubes: Any = None,
        write_cube_to_path: Any = None,
        write_cubes_to_paths: Any = None,
        suggest_version: Any = None,
        node_class_mappings: Any = None,
        node_class_mappings_provider: Any = None,
        retarget_cube_payload: Any = None,
        registry_factory: Any = None,
        git_runner: Any = None,
        preflight_service: Any = None,
    ) -> BackendServices:
        extension_root = tmp_path / "extension"
        extension_root.mkdir(exist_ok=True)
        (extension_root / "cubes").mkdir(exist_ok=True)
        tracked_repos = TrackedRepoService(
            extension_root,
            repositories=CommandRepository(git_runner) if git_runner else None,
            preflight_service=preflight_service or AllowingPreflightService(),
        )
        local_flavors = LocalFlavorService(tracked_repos)
        artifacts = CubeArtifactRepository(tracked_repos)
        history = CubeHistoryService(tracked_repos)
        redirects = CubeIdentityRedirectService(tracked_repos)
        identity = IdentityPolicyService(extension_root)
        ownership = OwnershipPolicyService(
            tracked_repo_service=tracked_repos,
            identity_policy_service=identity,
        )

        def default_prepared(
            loaded: object, drop_origin: tuple[float, float] = (0.0, 0.0)
        ) -> SimpleNamespace:
            return SimpleNamespace(
                nodes=[],
                markers=[],
                connections=[],
                layout=None,
                warnings=[],
                cube={},
                subgraphs=[],
            )

        library = CubeLibraryService(
            extension_root,
            load_cube_artifact=load_cube_artifact or (lambda path: None),
            prepare_cube_import=prepare_cube_import or default_prepared,
            tracked_repo_service=tracked_repos,
            ownership_policy_service=ownership,
            registry_factory=registry_factory,
        )
        workflow_catalog = WorkflowCatalogArtifactProvider(library.catalog_listing)
        workflow_library = CubeLibraryClassService(
            captured=CapturedCubeRepository(
                tracked_repos.data_root() / "captured_cubes"
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
        metadata = CubeMetadataService(
            library,
            retarget_cube_payload=(
                retarget_cube_payload
                or (
                    lambda payload, *, previous_cube_id, target_cube_id, previous_default_alias, target_default_alias: (
                        None
                    )
                )
            ),
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
            retarget_cube_payload=(
                retarget_cube_payload
                or (
                    lambda payload, *, previous_cube_id, target_cube_id, previous_default_alias, target_default_alias: (
                        None
                    )
                )
            ),
        )
        dependencies = CubeDependencyService(
            library_service=library,
            tracked_repo_service=tracked_repos,
            custom_nodes_root=tmp_path / "custom_nodes",
        )
        loader = CubeLoadService(
            library,
            load_cube_artifact=load_cube_artifact or (lambda path: None),
            prepare_cube_import=prepare_cube_import or default_prepared,
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
            export_cubes=export_cubes or (lambda *args, **kwargs: []),
            write_cube_to_path=write_cube_to_path or (lambda *args, **kwargs: {}),
            write_cubes_to_paths=write_cubes_to_paths or (lambda *args, **kwargs: []),
            suggest_version=suggest_version
            or (
                lambda existing, current: SimpleNamespace(
                    suggested="1.0.0", reason="same", bump="patch"
                )
            ),
            node_class_mappings_provider=(
                node_class_mappings_provider
                or (
                    lambda: (
                        {"KSampler": object()}
                        if node_class_mappings is None
                        else node_class_mappings
                    )
                )
            ),
            finalized_definition_provider=lambda _path, _cube_id, payload: {
                "cube": payload,
                "nodes": [],
                "markers": [],
                "connections": [],
                "layout": None,
                "warnings": [],
                "subgraphs": payload.get("implementation", {}).get("subgraphs", []),
            },
            local_flavor_service=local_flavors,
        )
        revisions = CubeRevisionService(
            library,
            tracked_repos,
            load_cube_artifact=load_cube_artifact or (lambda path: None),
            prepare_cube_import=prepare_cube_import or default_prepared,
            redirect_service=redirects,
        )

        def unavailable_artifact_loader(_path: Path) -> LoadedCube:
            """Fail clearly if an unrelated test invokes SugarScript resolution."""

            raise RuntimeError("Cube artifact loading is not configured for this test.")

        artifact_loader = cast(
            Callable[[Path], LoadedCube],
            load_cube_artifact or unavailable_artifact_loader,
        )
        workflow_resolver = SugarScriptCatalogResolver(
            library=library,
            redirects=redirects,
            load_cube=artifact_loader,
        )
        workflow_preparer = NativeCubeImportPreparerAdapter(
            load_document=lambda document: load_materialized_cube_document(document),
            prepare_import=lambda loaded: prepare_import(loaded),
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
        )

    return factory
