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
"""Tracked GitHub repo service tests."""

from __future__ import annotations

from typing import Any

import json
from dataclasses import dataclass
from pathlib import Path

import pytest

from sugarcubes.backend.responses import BackendError
from sugarcubes.backend.services import (
    IdentityPolicyService,
    OwnershipPolicyService,
    TrackedRepoPreflightResult,
    TrackedRepoService,
)
from sugarcubes.backend.services.tracked_repo_preflight_service import (
    HttpJsonResponse,
    TrackedRepoPreflightService,
)


@dataclass(frozen=True)
class _GitResult:
    """Provide the stdout surface consumed by temporary preflight fallback."""

    stdout: str = ""


def _unused_git_runner(args: list[str], *, cwd: Path) -> _GitResult:
    """Return an inert result for tests whose HTTP preflight never invokes git."""

    _ = args, cwd
    return _GitResult()


class AllowingPreflightService:
    """Permit tracked repo tests to opt out of remote GitHub calls."""

    def inspect_repo(self, *, owner: Any, repo: Any, branch: Any) -> Any:
        """Return one successful preflight result."""

        return TrackedRepoPreflightResult(
            owner=owner,
            repo=repo,
            branch=branch,
            contains_cubes=True,
            cube_count=1,
            cube_paths=("demo.cube",),
        )

    def require_cubes(self, *, owner: Any, repo: Any, branch: Any) -> Any:
        """Return one successful preflight result."""

        return self.inspect_repo(owner=owner, repo=repo, branch=branch)


def make_tracked_repo_service(
    extension_root: Any,
    *,
    git_runner: Any,
    preflight_service: Any = None,
    protected_owner_provider: Any = None,
) -> Any:
    """Create a tracked repo service with remote preflight isolated for tests."""

    return TrackedRepoService(
        extension_root,
        git_runner=git_runner,
        preflight_service=preflight_service or AllowingPreflightService(),
        protected_owner_provider=protected_owner_provider,
    )


def make_tree_response(paths: Any, *, truncated: Any = False, status: Any = 200) -> Any:
    """Build one fake GitHub tree response for preflight tests."""

    return HttpJsonResponse(
        status=status,
        headers={},
        payload={
            "truncated": truncated,
            "tree": [{"type": "blob", "path": path} for path in paths],
        },
    )


def test_preflight_succeeds_and_counts_nested_cube_paths(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = TrackedRepoPreflightService(
        workspace_root=extension_root / ".sugarcubes",
        git_runner=_unused_git_runner,
        http_json_loader=lambda url, headers, timeout: make_tree_response(
            ["demo.cube", "nested/alpha.cube", "notes/readme.md"]
        ),
    )

    result = service.require_cubes(
        owner="Artificial-Sweetener", repo="Base-Cubes", branch="main"
    )

    assert result.contains_cubes is True
    assert result.cube_count == 2
    assert result.cube_paths == ("demo.cube", "nested/alpha.cube")
    assert result.checked_via == "github_tree"


def test_preflight_ignores_backup_cube_paths(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = TrackedRepoPreflightService(
        workspace_root=extension_root / ".sugarcubes",
        git_runner=_unused_git_runner,
        http_json_loader=lambda url, headers, timeout: make_tree_response(
            [
                "old/demo.cube",
                "backup/demo.cube",
                "_old/demo.cube",
                "_history/demo.cube",
                "current/demo.cube",
            ]
        ),
    )

    result = service.require_cubes(
        owner="Artificial-Sweetener", repo="Base-Cubes", branch="main"
    )

    assert result.cube_count == 1
    assert result.cube_paths == ("current/demo.cube",)


def test_preflight_rejects_repo_without_cube_paths(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = TrackedRepoPreflightService(
        workspace_root=extension_root / ".sugarcubes",
        git_runner=_unused_git_runner,
        http_json_loader=lambda url, headers, timeout: make_tree_response(
            ["README.md", "workflow.json"]
        ),
    )

    with pytest.raises(BackendError) as error:
        service.require_cubes(
            owner="Artificial-Sweetener", repo="Base-Cubes", branch="main"
        )

    assert error.value.status == 422
    assert error.value.details["reason"] == "no_cubes"


def test_preflight_maps_github_404_and_rate_limit(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    missing = TrackedRepoPreflightService(
        workspace_root=extension_root / ".sugarcubes",
        git_runner=_unused_git_runner,
        http_json_loader=lambda url, headers, timeout: HttpJsonResponse(
            status=404, headers={}, payload={}
        ),
    )
    limited = TrackedRepoPreflightService(
        workspace_root=extension_root / ".sugarcubes",
        git_runner=_unused_git_runner,
        http_json_loader=lambda url, headers, timeout: HttpJsonResponse(
            status=403, headers={}, payload={}
        ),
    )

    with pytest.raises(BackendError) as missing_error:
        missing.require_cubes(
            owner="Artificial-Sweetener", repo="Missing", branch="main"
        )
    with pytest.raises(BackendError) as limited_error:
        limited.require_cubes(
            owner="Artificial-Sweetener", repo="Base-Cubes", branch="main"
        )

    assert missing_error.value.status == 404
    assert limited_error.value.status == 503


def test_preflight_uses_temporary_git_fallback_for_truncated_tree(
    tmp_path: Path,
) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))
        if args[:1] == ["clone"]:
            checkout = Path(args[-1])
            checkout.mkdir(parents=True, exist_ok=True)
            (checkout / "demo.cube").write_text("{}", encoding="utf-8")

        class Result:
            stdout = ""

        if args[:1] == ["clone"]:
            Path(args[-1]).mkdir(parents=True, exist_ok=True)
        elif args == ["ls-tree", "-r", "--name-only", "HEAD"]:
            Result.stdout = "demo.cube\nold/ignored.cube\nnested/second.cube\n"
        return Result()

    service = TrackedRepoPreflightService(
        workspace_root=extension_root / ".sugarcubes",
        git_runner=fake_git,
        http_json_loader=lambda url, headers, timeout: make_tree_response(
            [], truncated=True
        ),
    )

    result = service.require_cubes(
        owner="Artificial-Sweetener", repo="Base-Cubes", branch="main"
    )

    assert result.checked_via == "temporary_git_tree"
    assert result.truncated is True
    assert result.cube_count == 2
    assert not any((extension_root / ".sugarcubes" / "_preflight").glob("repo-*"))
    assert any(
        call[0][:6]
        == [
            "clone",
            "--depth=1",
            "--filter=blob:none",
            "--no-checkout",
            "--branch",
            "main",
        ]
        for call in calls
    )


def test_add_list_update_and_remove_tracked_repo(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=_unused_git_runner)

    initial = service.list_repos()
    assert initial["count"] == 1
    assert initial["repos"][0]["repo_ref"] == "Artificial-Sweetener/Base-Cubes"

    created = service.add_repo(
        owner="artificial-sweetener",
        repo="custom-cubes",
        branch="main",
        enabled=True,
        default_base_repo=False,
        auto_update=True,
    )
    assert created["repo"]["repo_ref"] == "artificial-sweetener/custom-cubes"
    assert created["repo"]["auto_update"] is True
    assert service.list_repos()["count"] == 2

    updated = service.update_repo(
        owner="artificial-sweetener",
        repo="custom-cubes",
        branch="release",
        enabled=False,
        auto_update=False,
    )
    assert updated["repo"]["branch"] == "main"
    assert updated["repo"]["enabled"] is False
    assert updated["repo"]["auto_update"] is False

    removed = service.remove_repo(owner="artificial-sweetener", repo="custom-cubes")
    assert removed["removed"] == {
        "owner": "artificial-sweetener",
        "repo": "custom-cubes",
    }
    assert service.list_repos()["count"] == 1


def test_add_repo_returns_preflight_summary(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=_unused_git_runner)

    created = service.add_repo(
        owner="artificial-sweetener",
        repo="custom-cubes",
        branch="main",
        enabled=True,
        default_base_repo=False,
    )

    assert created["preflight"]["contains_cubes"] is True
    assert created["preflight"]["cube_count"] == 1
    assert created["preflight"]["cube_paths"] == ["demo.cube"]


def test_add_repo_does_not_write_manifest_when_preflight_fails(tmp_path: Path) -> None:
    class DenyingPreflightService:
        """Reject every repo as containing no cubes."""

        def require_cubes(self, *, owner: Any, repo: Any, branch: Any) -> None:
            raise BackendError(
                f"Repository '{owner}/{repo}' does not contain any .cube files on branch '{branch}'.",
                status=422,
                details={
                    "repo": f"{owner}/{repo}",
                    "branch": branch,
                    "reason": "no_cubes",
                },
            )

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(
        extension_root,
        git_runner=_unused_git_runner,
        preflight_service=DenyingPreflightService(),
    )

    with pytest.raises(BackendError) as error:
        service.add_repo(
            owner="artificial-sweetener",
            repo="custom-cubes",
            branch="main",
            enabled=True,
            default_base_repo=False,
        )

    assert error.value.status == 422
    assert service.list_repos()["count"] == 1


def test_non_base_repo_cannot_become_default_base_repo(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=_unused_git_runner)

    created = service.add_repo(
        owner="artificial-sweetener",
        repo="custom-cubes",
        branch="main",
        enabled=True,
        default_base_repo=True,
        auto_update=False,
    )

    assert created["repo"]["default_base_repo"] is False
    listed = {entry["repo_ref"]: entry for entry in service.list_repos()["repos"]}
    assert listed["Artificial-Sweetener/Base-Cubes"]["default_base_repo"] is True
    assert listed["artificial-sweetener/custom-cubes"]["default_base_repo"] is False


def test_base_cubes_cannot_be_removed_or_disabled(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=_unused_git_runner)

    updated = service.update_repo(
        owner="Artificial-Sweetener",
        repo="Base-Cubes",
        enabled=False,
    )
    assert updated["repo"]["enabled"] is True

    with pytest.raises(BackendError, match="always tracked as the default base repo"):
        service.remove_repo(owner="Artificial-Sweetener", repo="Base-Cubes")


def test_manifest_load_repairs_missing_managed_checkout_path_after_folder_rename(
    tmp_path: Path,
) -> None:
    """Stale managed checkout paths should follow the active extension root."""

    extension_root = tmp_path / "SugarCubes"
    extension_root.mkdir()
    stale_checkout = (
        tmp_path
        / "ComfyUI-SugarCubes"
        / ".sugarcubes"
        / "Artificial-Sweetener"
        / "Base-Cubes"
    )
    service = make_tracked_repo_service(extension_root, git_runner=_unused_git_runner)
    service.manifest_path().parent.mkdir(parents=True, exist_ok=True)
    service.manifest_path().write_text(
        json.dumps(
            {
                "repos": [
                    {
                        "owner": "Artificial-Sweetener",
                        "repo": "Base-Cubes",
                        "branch": "main",
                        "enabled": True,
                        "default_base_repo": True,
                        "local_checkout_path": str(stale_checkout),
                    }
                ]
            }
        ),
        encoding="utf-8",
    )

    listed = service.list_repos()

    expected_checkout = service.checkout_path("Artificial-Sweetener", "Base-Cubes")
    assert listed["repos"][0]["local_checkout_path"] == str(expected_checkout)
    persisted = json.loads(service.manifest_path().read_text(encoding="utf-8"))
    assert persisted["repos"][0]["local_checkout_path"] == str(expected_checkout)


def test_ensure_authoring_repo_initializes_checkout_git_repo(tmp_path: Path) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))
        if args[:2] == ["init", "-b"]:
            (Path(cwd) / ".git").mkdir(parents=True)

        class Result:
            stdout = ""

        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)

    created = service.ensure_authoring_repo(owner="ExampleUser", repo="Example-Cubes")

    checkout = service.checkout_path("ExampleUser", "Example-Cubes")
    assert created["repo"]["repo_ref"] == "ExampleUser/Example-Cubes"
    assert created["repo"]["enabled"] is True
    assert (checkout / ".git").exists()
    assert calls == [(["init", "-b", "main"], checkout)]


def test_ensure_authoring_repo_reuses_existing_enabled_repo(tmp_path: Path) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))
        if args[:1] == ["clone"]:
            checkout = Path(args[-1])
            checkout.mkdir(parents=True, exist_ok=True)
            (checkout / "demo.cube").write_text("{}", encoding="utf-8")

        class Result:
            stdout = ""

        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)
    service.add_repo(
        owner="ExampleUser",
        repo="Example-Cubes",
        branch="main",
        enabled=True,
        default_base_repo=False,
    )
    checkout = service.checkout_path("ExampleUser", "Example-Cubes")
    (checkout / ".git").mkdir(parents=True)

    existing = service.ensure_authoring_repo(owner="ExampleUser", repo="Example-Cubes")

    assert existing["repo"]["repo_ref"] == "ExampleUser/Example-Cubes"
    assert service.list_repos()["count"] == 2
    assert calls == []


def test_ensure_authoring_repo_rejects_disabled_existing_repo(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=_unused_git_runner)
    service.add_repo(
        owner="ExampleUser",
        repo="Example-Cubes",
        branch="main",
        enabled=False,
        default_base_repo=False,
    )

    with pytest.raises(BackendError, match="is disabled"):
        service.ensure_authoring_repo(owner="ExampleUser", repo="Example-Cubes")


def test_sync_repo_clones_when_checkout_is_missing(tmp_path: Path) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))
        if args[:1] == ["clone"]:
            checkout = Path(args[-1])
            checkout.mkdir(parents=True, exist_ok=True)
            (checkout / "demo.cube").write_text("{}", encoding="utf-8")

        class Result:
            stdout = ""

        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)

    synced = service.sync_repo(owner="Artificial-Sweetener", repo="Base-Cubes")

    assert synced["repo"]["last_sync_status"] == "ok"
    assert calls[0][0][:3] == ["clone", "--branch", "main"]


def test_sync_repo_falls_back_to_plain_clone_for_empty_remote(tmp_path: Path) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))
        if args[:3] == ["clone", "--branch", "main"]:
            raise RuntimeError("Remote branch main not found in upstream origin")
        if args[:1] == ["clone"]:
            checkout = Path(args[-1])
            checkout.mkdir(parents=True, exist_ok=True)
            (checkout / "demo.cube").write_text("{}", encoding="utf-8")

        class Result:
            stdout = ""

        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)

    synced = service.sync_repo(owner="Artificial-Sweetener", repo="Base-Cubes")

    assert synced["repo"]["last_sync_status"] == "ok"
    assert calls[0][0][:3] == ["clone", "--branch", "main"]
    assert calls[1][0][0] == "clone"
    assert "--branch" not in calls[1][0]


def test_sync_repo_rejects_dirty_worktree(tmp_path: Path) -> None:
    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = " M demo.cube" if args == ["status", "--porcelain"] else ""

        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)
    checkout = service.checkout_path("Artificial-Sweetener", "Base-Cubes")
    checkout.mkdir(parents=True)

    with pytest.raises(BackendError, match="local changes"):
        service.sync_repo(owner="Artificial-Sweetener", repo="Base-Cubes")


def test_sync_repo_rejects_claimed_author_repo_with_local_commits_ahead(
    tmp_path: Path,
) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))
        if args[:3] == ["merge-base", "--is-ancestor", "HEAD"]:
            raise RuntimeError("not ancestor")

        class Result:
            stdout = ""

        if args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["rev-parse", "HEAD"]:
            Result.stdout = "local-sha\n"
        elif args == ["rev-parse", "origin/main"]:
            Result.stdout = "remote-sha\n"
        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(
        extension_root,
        git_runner=fake_git,
        protected_owner_provider=lambda: "Artificial-Sweetener",
    )
    checkout = service.checkout_path("Artificial-Sweetener", "Base-Cubes")
    checkout.mkdir(parents=True)
    (checkout / "demo.cube").write_text("{}", encoding="utf-8")

    with pytest.raises(BackendError, match="local commits are ahead"):
        service.sync_repo(owner="Artificial-Sweetener", repo="Base-Cubes")

    assert not any(call[0][:2] == ["reset", "--hard"] for call in calls)


def test_sync_repo_rejects_default_base_repo_with_local_commits_ahead(
    tmp_path: Path,
) -> None:
    """Base-Cubes sync must not discard local repair commits."""

    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))
        if args[:3] == ["merge-base", "--is-ancestor", "HEAD"]:
            raise RuntimeError("not ancestor")

        class Result:
            stdout = ""

        if args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["rev-parse", "HEAD"]:
            Result.stdout = "local-sha\n"
        elif args == ["rev-parse", "origin/main"]:
            Result.stdout = "remote-sha\n"
        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(
        extension_root,
        git_runner=fake_git,
        protected_owner_provider=lambda: "ExampleUser",
    )
    checkout = service.checkout_path("Artificial-Sweetener", "Base-Cubes")
    checkout.mkdir(parents=True)
    (checkout / "demo.cube").write_text("{}", encoding="utf-8")

    with pytest.raises(BackendError, match="local commits are ahead"):
        service.sync_repo(owner="Artificial-Sweetener", repo="Base-Cubes")

    assert not any(call[0][:2] == ["reset", "--hard"] for call in calls)


def test_sync_repo_allows_nondefault_external_repo_with_local_commits_ahead(
    tmp_path: Path,
) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))

        class Result:
            stdout = ""

        if args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["rev-parse", "HEAD"]:
            Result.stdout = "remote-sha\n"
        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(
        extension_root,
        git_runner=fake_git,
        protected_owner_provider=lambda: "ExampleUser",
    )
    service.add_repo(
        owner="Artificial-Sweetener",
        repo="External-Cubes",
        branch="main",
        enabled=True,
        default_base_repo=False,
    )
    checkout = service.checkout_path("Artificial-Sweetener", "External-Cubes")
    checkout.mkdir(parents=True)
    (checkout / "demo.cube").write_text("{}", encoding="utf-8")

    synced = service.sync_repo(owner="Artificial-Sweetener", repo="External-Cubes")

    assert synced["repo"]["last_sync_status"] == "ok"
    assert any(call[0][:2] == ["reset", "--hard"] for call in calls)


def test_sync_repo_rejects_checkout_without_cubes(tmp_path: Path) -> None:
    def fake_git(args: Any, *, cwd: Any) -> Any:
        class Result:
            stdout = ""

        if args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["rev-parse", "HEAD"]:
            Result.stdout = "abc123\n"
        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)
    checkout = service.checkout_path("Artificial-Sweetener", "Base-Cubes")
    checkout.mkdir(parents=True)
    (checkout / "README.md").write_text("not a cube", encoding="utf-8")

    with pytest.raises(BackendError) as error:
        service.sync_repo(owner="Artificial-Sweetener", repo="Base-Cubes")

    assert error.value.status == 422
    assert error.value.details["reason"] == "no_cubes"
    repo = service.get_repo("Artificial-Sweetener", "Base-Cubes")
    assert repo.last_sync_status == "error"
    assert "does not contain any .cube files" in repo.last_sync_error


def test_add_repo_rejects_reserved_local_owner(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=_unused_git_runner)

    with pytest.raises(BackendError, match="reserved"):
        service.add_repo(
            owner="local",
            repo="shadow",
            branch="main",
            enabled=True,
            default_base_repo=False,
        )


def test_ensure_local_repo_initializes_main_branch_git_repo(tmp_path: Path) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))

        class Result:
            stdout = ""

        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)

    local_root = service.ensure_local_repo()

    assert local_root == extension_root / ".sugarcubes" / "local"
    assert local_root.exists()
    assert calls == [(["init", "-b", "main"], local_root)]


def test_check_repo_persists_update_metadata_without_mutating_checkout(
    tmp_path: Path,
) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))

        class Result:
            stdout = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = "abc123\n"
        elif args[:2] == ["ls-remote", "--heads"]:
            Result.stdout = "def456\trefs/heads/main\n"
        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)
    checkout = service.checkout_path("Artificial-Sweetener", "Base-Cubes")
    checkout.mkdir(parents=True)

    checked = service.check_repo(owner="Artificial-Sweetener", repo="Base-Cubes")

    assert checked["repo"]["local_head_sha"] == "abc123"
    assert checked["repo"]["remote_head_sha"] == "def456"
    assert checked["repo"]["update_available"] is True
    assert checked["repo"]["last_check_status"] == "ok"
    assert not any(call[0][0] == "fetch" for call in calls)
    assert not any(call[0][0] == "reset" for call in calls)


def test_check_all_repos_applies_auto_update_only_to_enabled_packs(
    tmp_path: Path,
) -> None:
    calls = []

    def fake_git(args: Any, *, cwd: Any) -> Any:
        calls.append((list(args), Path(cwd)))

        class Result:
            stdout = ""

        if args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["rev-parse", "HEAD"]:
            Result.stdout = "updatedsha\n"
        elif args[:2] == ["ls-remote", "--heads"]:
            Result.stdout = "remotesha\trefs/heads/main\n"
        return Result()

    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = make_tracked_repo_service(extension_root, git_runner=fake_git)
    service.add_repo(
        owner="artificial-sweetener",
        repo="custom-cubes",
        branch="main",
        enabled=True,
        default_base_repo=False,
        auto_update=True,
    )
    disabled = service.add_repo(
        owner="artificial-sweetener",
        repo="disabled-pack",
        branch="main",
        enabled=False,
        default_base_repo=False,
        auto_update=True,
    )["repo"]
    service.checkout_path("Artificial-Sweetener", "Base-Cubes").mkdir(parents=True)
    custom_checkout = service.checkout_path("artificial-sweetener", "custom-cubes")
    custom_checkout.mkdir(parents=True)
    (custom_checkout / "demo.cube").write_text("{}", encoding="utf-8")

    payload = service.check_all_repos(apply_auto_updates=True)

    by_ref = {repo["repo_ref"]: repo for repo in payload["repos"]}
    assert by_ref["Artificial-Sweetener/Base-Cubes"]["last_sync_status"] == "never"
    assert by_ref["artificial-sweetener/custom-cubes"]["last_sync_status"] == "ok"
    assert by_ref[disabled["repo_ref"]]["enabled"] is False
    reset_calls = [call for call in calls if call[0][:2] == ["reset", "--hard"]]
    assert len(reset_calls) == 1


def test_identity_policy_persists_single_claimed_owner(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = IdentityPolicyService(extension_root)

    initial = service.get_policy()
    assert initial.claimed_github_owner == ""
    assert initial.allow_system_owner_claim is False

    updated = service.set_policy(claimed_github_owner="ExampleUser")

    assert updated["claimed_github_owner"] == "ExampleUser"
    assert updated["has_claimed_github_owner"] is True
    persisted = service.get_policy()
    assert persisted.claimed_github_owner == "ExampleUser"


def test_identity_policy_rejects_reserved_local_owner(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = IdentityPolicyService(extension_root)

    with pytest.raises(BackendError, match="reserved"):
        service.set_policy(claimed_github_owner="local")


def test_identity_policy_blocks_system_owner_without_env_gate(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    service = IdentityPolicyService(extension_root)

    updated = service.set_policy(claimed_github_owner="artificial-sweetener")

    assert updated["allow_system_owner_claim"] is False
    assert updated["claimed_github_owner"] == ""


def test_identity_policy_ignores_file_backed_system_owner_gate(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    policy_path = extension_root / ".sugarcubes" / "identity_policy.json"
    policy_path.parent.mkdir(parents=True, exist_ok=True)
    policy_path.write_text(
        '{\n  "claimed_github_owner": "Artificial-Sweetener",\n  "allow_system_owner_claim": true\n}\n',
        encoding="utf-8",
    )
    service = IdentityPolicyService(extension_root)

    policy = service.get_policy()

    assert policy.claimed_github_owner == ""
    assert policy.allow_system_owner_claim is False
    assert policy.claimed_github_owner_source == "default"
    assert policy.allow_system_owner_claim_source == "default"
    assert (
        policy_path.read_text(encoding="utf-8")
        == '{\n  "claimed_github_owner": ""\n}\n'
    )


def test_identity_policy_reads_repo_root_dotenv_override(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    (extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n"
        "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
        encoding="utf-8",
    )
    service = IdentityPolicyService(extension_root)

    policy = service.get_policy()

    assert policy.claimed_github_owner == "Artificial-Sweetener"
    assert policy.allow_system_owner_claim is True
    assert policy.claimed_github_owner_source == "dotenv"
    assert policy.allow_system_owner_claim_source == "dotenv"
    assert policy.env_override_active is True


def test_identity_policy_process_env_overrides_dotenv(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    (extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n"
        "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
        encoding="utf-8",
    )
    monkeypatch.setenv("SUGARCUBES_CLAIMED_GITHUB_OWNER", "ExampleUser")
    monkeypatch.setenv("SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM", "0")
    service = IdentityPolicyService(extension_root)

    policy = service.get_policy()

    assert policy.claimed_github_owner == "ExampleUser"
    assert policy.allow_system_owner_claim is False
    assert policy.claimed_github_owner_source == "process_env"
    assert policy.allow_system_owner_claim_source == "process_env"


def test_identity_policy_env_system_owner_still_requires_gate(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    (extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n"
        "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=0\n",
        encoding="utf-8",
    )
    service = IdentityPolicyService(extension_root)

    policy = service.get_policy()

    assert policy.claimed_github_owner == ""
    assert policy.allow_system_owner_claim is False
    assert policy.claimed_github_owner_source == "dotenv"
    assert policy.allow_system_owner_claim_source == "dotenv"


def test_identity_policy_rejects_updates_to_env_managed_fields(tmp_path: Path) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    (extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n"
        "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
        encoding="utf-8",
    )
    service = IdentityPolicyService(extension_root)

    with pytest.raises(BackendError, match="managed by environment configuration"):
        service.set_policy(claimed_github_owner="ExampleUser")


def test_ownership_policy_marks_local_cubes_writable_without_claim(
    tmp_path: Path,
) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    tracked_repos = make_tracked_repo_service(
        extension_root, git_runner=_unused_git_runner
    )
    identity = IdentityPolicyService(extension_root)
    ownership = OwnershipPolicyService(
        tracked_repo_service=tracked_repos,
        identity_policy_service=identity,
    )

    payload = ownership.describe_source(
        source_kind="local",
        owner="",
        repo="",
        namespace="example-user",
        require_tracked_repo=False,
    )

    assert payload["is_writable"] is True
    assert payload["write_target_kind"] == "local"


def test_ownership_policy_requires_matching_claim_and_tracked_repo(
    tmp_path: Path,
) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    tracked_repos = make_tracked_repo_service(
        extension_root, git_runner=_unused_git_runner
    )
    tracked_repos.add_repo(
        owner="example-user",
        repo="my-pack",
        branch="main",
        enabled=True,
        default_base_repo=False,
    )
    identity = IdentityPolicyService(extension_root)
    identity.set_policy(claimed_github_owner="example-user")
    ownership = OwnershipPolicyService(
        tracked_repo_service=tracked_repos,
        identity_policy_service=identity,
    )

    owned = ownership.describe_tracked_repo(owner="example-user", repo="my-pack")
    external = ownership.describe_tracked_repo(
        owner="Artificial-Sweetener",
        repo="Base-Cubes",
    )

    assert owned["is_writable"] is True
    assert owned["ownership_mode"] == "mine"
    assert external["is_writable"] is False
    assert "does not match" in external["write_block_reason"]


def test_ownership_policy_blocks_system_owner_without_advanced_gate(
    tmp_path: Path,
) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    tracked_repos = make_tracked_repo_service(
        extension_root, git_runner=_unused_git_runner
    )
    identity = IdentityPolicyService(extension_root)
    ownership = OwnershipPolicyService(
        tracked_repo_service=tracked_repos,
        identity_policy_service=identity,
    )

    payload = ownership.describe_tracked_repo(
        owner="Artificial-Sweetener",
        repo="Base-Cubes",
    )

    assert payload["is_system_pack"] is True
    assert payload["is_writable"] is False


def test_ownership_policy_uses_env_managed_owner_for_tracked_repo_writes(
    tmp_path: Path,
) -> None:
    extension_root = tmp_path / "extension"
    extension_root.mkdir()
    (extension_root / ".env").write_text(
        "SUGARCUBES_CLAIMED_GITHUB_OWNER=Artificial-Sweetener\n"
        "SUGARCUBES_ALLOW_SYSTEM_OWNER_CLAIM=1\n",
        encoding="utf-8",
    )
    tracked_repos = make_tracked_repo_service(
        extension_root, git_runner=_unused_git_runner
    )
    identity = IdentityPolicyService(extension_root)
    ownership = OwnershipPolicyService(
        tracked_repo_service=tracked_repos,
        identity_policy_service=identity,
    )

    payload = ownership.describe_tracked_repo(
        owner="Artificial-Sweetener",
        repo="Base-Cubes",
    )

    assert payload["is_writable"] is True
    assert payload["ownership_mode"] == "mine"
