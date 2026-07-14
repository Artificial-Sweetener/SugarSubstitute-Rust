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
import sys
import types
import json
from pathlib import Path
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[1]

if "sugarcubes" not in sys.modules:
    package = types.ModuleType("sugarcubes")
    package.__path__ = [str(ROOT)]
    sys.modules["sugarcubes"] = package

collect_ignore = [str(ROOT / "__init__.py")]


class FakeRequest:
    """Minimal aiohttp-like request object for route handler tests."""

    def __init__(self, *, body=None, query=None, json_error=None):
        self._body = body
        self.query = query or {}
        self._json_error = json_error

    async def json(self):
        if self._json_error is not None:
            raise self._json_error
        return self._body


def decode_json_response(response):
    """Decode an aiohttp JSON response body for assertions."""

    return json.loads(response.body.decode("utf-8"))


class AllowingPreflightService:
    """Allow backend tests to avoid live GitHub preflight calls by default."""

    def inspect_repo(self, *, owner, repo, branch):
        """Return a successful preflight result for one test repo."""

        from sugarcubes.backend.services import TrackedRepoPreflightResult

        return TrackedRepoPreflightResult(
            owner=owner,
            repo=repo,
            branch=branch,
            contains_cubes=True,
            cube_count=1,
            cube_paths=("demo.cube",),
        )

    def require_cubes(self, *, owner, repo, branch):
        """Return a successful preflight result for one test repo."""

        return self.inspect_repo(owner=owner, repo=repo, branch=branch)


def ensure_tracked_repo(
    services,
    *,
    owner="Artificial-Sweetener",
    repo="Base-Cubes",
    branch="main",
    enabled=True,
    default_base_repo=True,
):
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
    services,
    *,
    owner,
    allow_system_owner_claim=False,
):
    """Persist one claimed GitHub owner for backend ownership tests."""

    if allow_system_owner_claim:
        (services.identity.extension_root / ".env").write_text(
            "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
            encoding="utf-8",
        )
    services.ownership.update_identity_policy(claimed_github_owner=owner)


@pytest.fixture
def backend_services_factory():
    """Build isolated backend services for route and service tests."""

    from sugarcubes.backend import BackendServices
    from sugarcubes.backend.services import (
        CubeArtifactRepository,
        CubeExportService,
        CubeDependencyService,
        CubeHistoryService,
        CubeIdentityRedirectService,
        CubeLibraryService,
        CubeLoadService,
        CubeMetadataService,
        CubePromotionService,
        CubeRevisionService,
        IdentityPolicyService,
        LocalFlavorService,
        OwnershipPolicyService,
        TrackedRepoService,
    )

    def factory(
        tmp_path,
        *,
        load_cube_artifact=None,
        prepare_cube_import=None,
        export_cubes=None,
        write_cube=None,
        write_cube_to_path=None,
        write_cubes=None,
        write_cubes_to_paths=None,
        suggest_version=None,
        node_class_mappings=None,
        node_class_mappings_provider=None,
        retarget_cube_payload=None,
        registry_factory=None,
        git_runner=None,
        preflight_service=None,
    ):
        extension_root = tmp_path / "extension"
        extension_root.mkdir(exist_ok=True)
        (extension_root / "cubes").mkdir(exist_ok=True)
        tracked_repos = TrackedRepoService(
            extension_root,
            git_runner=git_runner,
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

        def default_prepared(loaded, drop_origin=(0.0, 0.0)):
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
            workspace_path=tmp_path / "ComfyUI",
            custom_nodes_root=tmp_path / "custom_nodes",
        )
        loader = CubeLoadService(
            library,
            load_cube_artifact=load_cube_artifact or (lambda path: None),
            prepare_cube_import=prepare_cube_import or default_prepared,
            redirect_service=redirects,
        )
        exporter = CubeExportService(
            library,
            export_cubes=export_cubes or (lambda *args, **kwargs: []),
            write_cube=write_cube or (lambda *args, **kwargs: {}),
            write_cube_to_path=write_cube_to_path or (lambda *args, **kwargs: {}),
            write_cubes=write_cubes or (lambda *args, **kwargs: []),
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
        return BackendServices(
            library=library,
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
        )

    return factory
