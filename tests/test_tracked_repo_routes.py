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
"""Tracked repo and identity policy route tests."""

from __future__ import annotations

from typing import Any

from pathlib import Path

from .typing_support import BackendServicesFactory

import asyncio

from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.backend.responses import BackendError
from sugarcubes.backend.services import TrackedRepoPreflightResult

from .conftest import FakeRequest, claim_github_owner, decode_json_response


class FixedPreflightService:
    """Return a deterministic preflight result for route tests."""

    def __init__(self, *, result: Any = None, error: Any = None) -> None:
        self.result = result or TrackedRepoPreflightResult(
            owner="artificial-sweetener",
            repo="custom-cubes",
            branch="main",
            contains_cubes=True,
            cube_count=2,
            cube_paths=("demo.cube", "nested/alpha.cube"),
        )
        self.error = error

    def inspect_repo(self, *, owner: Any, repo: Any, branch: Any) -> Any:
        """Return or raise the configured preflight result."""

        if self.error:
            raise self.error
        return TrackedRepoPreflightResult(
            owner=owner,
            repo=repo,
            branch=branch,
            contains_cubes=self.result.contains_cubes,
            cube_count=self.result.cube_count,
            cube_paths=self.result.cube_paths,
            checked_via=self.result.checked_via,
        )

    def require_cubes(self, *, owner: Any, repo: Any, branch: Any) -> Any:
        """Return or raise the configured preflight result."""

        return self.inspect_repo(owner=owner, repo=repo, branch=branch)


def test_tracked_repo_routes_cover_crud_check_and_sync(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    git_calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        git_calls.append((list(args), str(cwd)))
        if args[:1] == ["clone"]:
            checkout = args[-1]
            from pathlib import Path

            Path(checkout).mkdir(parents=True, exist_ok=True)
            (Path(checkout) / "demo.cube").write_text("{}", encoding="utf-8")

        class Result:
            stdout = ""

        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    handlers = build_route_handlers(services)

    create_response = asyncio.run(
        handlers.add_tracked_repo(
            FakeRequest(
                body={
                    "owner": "artificial-sweetener",
                    "repo": "custom-cubes",
                    "branch": "release",
                    "enabled": True,
                    "default_base_repo": False,
                    "auto_update": True,
                }
            )
        )
    )
    create_payload = decode_json_response(create_response)
    assert create_response.status == 201
    assert create_payload["repo"]["repo_ref"] == "artificial-sweetener/custom-cubes"
    assert create_payload["repo"]["branch"] == "main"
    assert create_payload["preflight"]["contains_cubes"] is True

    list_response = asyncio.run(handlers.list_tracked_repos(FakeRequest()))
    list_payload = decode_json_response(list_response)
    assert list_response.status == 200
    assert list_payload["count"] == 2

    check_response = asyncio.run(
        handlers.check_tracked_repo(
            FakeRequest(body={"owner": "artificial-sweetener", "repo": "custom-cubes"})
        )
    )
    check_payload = decode_json_response(check_response)
    assert check_response.status == 200
    assert "update_available" in check_payload["repo"]

    check_all_response = asyncio.run(
        handlers.check_all_tracked_repos(
            FakeRequest(body={"apply_auto_updates": False})
        )
    )
    check_all_payload = decode_json_response(check_all_response)
    assert check_all_response.status == 200
    assert check_all_payload["count"] == 2

    sync_response = asyncio.run(
        handlers.sync_tracked_repo(
            FakeRequest(body={"owner": "artificial-sweetener", "repo": "custom-cubes"})
        )
    )
    sync_payload = decode_json_response(sync_response)
    assert sync_response.status == 200
    assert sync_payload["repo"]["last_sync_status"] == "ok"
    assert any(call[0][:3] == ["clone", "--branch", "main"] for call in git_calls)

    update_response = asyncio.run(
        handlers.update_tracked_repo(
            FakeRequest(
                body={
                    "owner": "artificial-sweetener",
                    "repo": "custom-cubes",
                    "branch": "develop",
                    "enabled": False,
                    "auto_update": False,
                }
            )
        )
    )
    update_payload = decode_json_response(update_response)
    assert update_response.status == 200
    assert update_payload["repo"]["branch"] == "main"
    assert update_payload["repo"]["enabled"] is False
    assert update_payload["repo"]["auto_update"] is False

    remove_response = asyncio.run(
        handlers.remove_tracked_repo(
            FakeRequest(query={"owner": "artificial-sweetener", "repo": "custom-cubes"})
        )
    )
    remove_payload = decode_json_response(remove_response)
    assert remove_response.status == 200
    assert remove_payload["removed"]["repo"] == "custom-cubes"


def test_preflight_tracked_repo_route_does_not_write_manifest(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(
        tmp_path,
        git_runner=lambda args, cwd: None,
        preflight_service=FixedPreflightService(),
    )
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.preflight_tracked_repo(
            FakeRequest(body={"owner": "artificial-sweetener", "repo": "custom-cubes"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["preflight"]["cube_count"] == 2
    assert payload["preflight"]["cube_paths"] == ["demo.cube", "nested/alpha.cube"]
    assert services.tracked_repos.list_repos()["count"] == 1


def test_add_tracked_repo_route_rejects_no_cube_repo_before_manifest_write(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(
        tmp_path,
        git_runner=lambda args, cwd: None,
        preflight_service=FixedPreflightService(
            error=BackendError(
                "Repository 'artificial-sweetener/empty-pack' does not contain any .cube files on branch 'main'.",
                status=422,
                details={
                    "repo": "artificial-sweetener/empty-pack",
                    "branch": "main",
                    "reason": "no_cubes",
                },
            )
        ),
    )
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.add_tracked_repo(
            FakeRequest(
                body={
                    "owner": "artificial-sweetener",
                    "repo": "empty-pack",
                    "branch": "main",
                    "enabled": True,
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 422
    assert payload["error"]["details"]["reason"] == "no_cubes"
    assert services.tracked_repos.list_repos()["count"] == 1


def test_route_ignores_attempts_to_change_default_base_repo(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    handlers = build_route_handlers(services)

    create_response = asyncio.run(
        handlers.add_tracked_repo(
            FakeRequest(
                body={
                    "owner": "artificial-sweetener",
                    "repo": "custom-cubes",
                    "branch": "main",
                    "enabled": True,
                    "default_base_repo": True,
                }
            )
        )
    )
    create_payload = decode_json_response(create_response)
    assert create_payload["repo"]["default_base_repo"] is False

    update_response = asyncio.run(
        handlers.update_tracked_repo(
            FakeRequest(
                body={
                    "owner": "artificial-sweetener",
                    "repo": "custom-cubes",
                    "default_base_repo": True,
                }
            )
        )
    )
    update_payload = decode_json_response(update_response)
    assert update_payload["repo"]["default_base_repo"] is False


def test_authoring_repo_route_creates_writable_checkout(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    git_calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        git_calls.append((list(args), str(cwd)))

        class Result:
            stdout = ""

        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    claim_github_owner(services, owner="ExampleUser")
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.create_authoring_repo(
            FakeRequest(body={"owner": "ExampleUser", "repo": "Example-Cubes"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 201
    assert payload["repo"]["repo_ref"] == "ExampleUser/Example-Cubes"
    assert payload["repo"]["is_writable"] is True
    assert payload["repo"]["ownership_mode"] == "mine"
    assert payload["repo"]["write_target_kind"] == "tracked_owned_repo"
    assert git_calls == [
        (
            ["init", "-b", "main"],
            str(services.tracked_repos.checkout_path("ExampleUser", "Example-Cubes")),
        )
    ]


def test_authoring_repo_route_rejects_without_claim(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.create_authoring_repo(
            FakeRequest(body={"owner": "ExampleUser", "repo": "Example-Cubes"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 403
    assert "read-only until you claim" in payload["error"]["message"]


def test_authoring_repo_route_rejects_mismatched_owner(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    claim_github_owner(services, owner="OtherOwner")
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.create_authoring_repo(
            FakeRequest(body={"owner": "ExampleUser", "repo": "Example-Cubes"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 403
    assert "does not match" in payload["error"]["message"]


def test_authoring_repo_route_respects_system_owner_gate(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    (services.identity.extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n",
        encoding="utf-8",
    )
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.create_authoring_repo(
            FakeRequest(body={"owner": "Artificial-Sweetener", "repo": "Base-Cubes"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 403
    assert "read-only until you claim" in payload["error"]["message"]


def test_identity_policy_routes_persist_claimed_owner(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    handlers = build_route_handlers(services)

    update_response = asyncio.run(
        handlers.update_identity_policy(
            FakeRequest(
                body={
                    "claimed_github_owner": "example-user",
                }
            )
        )
    )
    update_payload = decode_json_response(update_response)

    assert update_response.status == 200
    assert update_payload["claimed_github_owner"] == "example-user"
    assert update_payload["allow_system_owner_claim"] is False

    get_response = asyncio.run(handlers.get_identity_policy(FakeRequest()))
    get_payload = decode_json_response(get_response)

    assert get_response.status == 200
    assert get_payload["claimed_github_owner"] == "example-user"


def test_identity_policy_route_reports_env_managed_sources(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    (services.identity.extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n"
        "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
        encoding="utf-8",
    )
    handlers = build_route_handlers(services)

    response = asyncio.run(handlers.get_identity_policy(FakeRequest()))
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["claimed_github_owner"] == "Artificial-Sweetener"
    assert payload["allow_system_owner_claim"] is True
    assert payload["claimed_github_owner_source"] == "dotenv"
    assert payload["allow_system_owner_claim_source"] == "dotenv"
    assert payload["env_override_active"] is True


def test_identity_policy_route_rejects_file_backed_system_owner_gate_updates(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.update_identity_policy(
            FakeRequest(body={"allow_system_owner_claim": True})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 400
    assert "managed only by environment configuration" in payload["error"]["message"]


def test_identity_policy_route_rejects_updates_to_env_managed_fields(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    (services.identity.extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n"
        "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
        encoding="utf-8",
    )
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.update_identity_policy(
            FakeRequest(body={"claimed_github_owner": "ExampleUser"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 409
    assert "managed by environment configuration" in payload["error"]["message"]


def test_identity_policy_route_blocks_system_owner_without_gate(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    handlers = build_route_handlers(services)

    response = asyncio.run(
        handlers.update_identity_policy(
            FakeRequest(body={"claimed_github_owner": "local"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 400
    assert "reserved" in payload["error"]["message"]


def test_tracked_repo_list_marks_base_pack_writable_from_env_managed_identity(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    (services.identity.extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n"
        "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
        encoding="utf-8",
    )
    handlers = build_route_handlers(services)

    response = asyncio.run(handlers.list_tracked_repos(FakeRequest()))
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["identity_policy"]["env_override_active"] is True
    base_repo = next(
        entry
        for entry in payload["repos"]
        if entry["repo_ref"] == "Artificial-Sweetener/Base-Cubes"
    )
    assert base_repo["is_writable"] is True
    assert base_repo["ownership_mode"] == "mine"
