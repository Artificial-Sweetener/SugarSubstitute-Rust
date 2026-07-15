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
"""Cube dependency repair service tests."""

from __future__ import annotations

from typing import Any

from .typing_support import BackendServicesFactory

import json
import subprocess
from pathlib import Path
from typing import Sequence

from sugarcubes.backend.services.cube_dependency_service import (
    ComfyCliAdapter,
    CubeDependencyService,
)

from .test_cube_library_backend_contract import _cube_payload_with_cnr, _write_cube


def _completed(
    command: Sequence[str], return_code: int = 0
) -> subprocess.CompletedProcess[str]:
    """Return one fake completed subprocess."""

    return subprocess.CompletedProcess(
        list(command),
        return_code,
        stdout="ok",
        stderr="" if return_code == 0 else "failed",
    )


def _recorded_completed(
    commands: list[list[str]], command: Sequence[str]
) -> subprocess.CompletedProcess[str]:
    """Record one command and return a successful subprocess result."""

    commands.append(list(command))
    return _completed(command)


def test_repair_installs_baseline_nodes_without_prompt(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Base-Cubes-only dependency repair runs under the silent baseline policy."""

    commands: list[list[str]] = []

    def runner(
        command: Sequence[str], cwd: Path, timeout_seconds: int
    ) -> subprocess.CompletedProcess[str]:
        _ = cwd, timeout_seconds
        commands.append(list(command))
        return _completed(command)

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=tmp_path / "custom_nodes",
        cli_adapter=ComfyCliAdapter(
            python_executable=tmp_path / "ComfyUI" / "venv" / "Scripts" / "python.exe",
            runner=runner,
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["installedNodes"][0]["nodeId"] == "comfyui-impact-pack"
    assert commands[0][-1] == "import comfy_cli"
    assert commands[1][-4:] == [
        "node",
        "install",
        "--exit-on-fail",
        "comfyui-impact-pack",
    ]
    assert "--workspace" in commands[1]
    assert "--skip-prompt" in commands[1]


def test_repair_refuses_non_default_nodes_without_approval(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Non-default cube pack dependencies are skipped until approved."""

    commands: list[list[str]] = []
    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    services.tracked_repos.add_repo(
        owner="Example",
        repo="Cubes",
        branch="main",
        enabled=True,
        default_base_repo=False,
    )
    checkout = services.tracked_repos.checkout_path("Example", "Cubes")
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cube_id="Example/Cubes/demo.cube",
            cnr_id="comfyui-example",
            python_module="custom_nodes.comfyui-example",
        ),
    )
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=tmp_path / "custom_nodes",
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _recorded_completed(
                commands, command
            ),
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["installedNodes"] == []
    assert result["skippedNodes"][0]["nodeId"] == "comfyui-example"
    assert commands == []


def test_repair_reports_missing_comfy_cli_without_raising(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Missing Comfy CLI is reported as failed repair work."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=tmp_path / "custom_nodes",
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _completed(command, 1),
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["installedNodes"] == []
    assert result["failedNodes"][0]["nodeId"] == "comfyui-impact-pack"
    assert result["failedNodes"][0]["reason"] == "missing_comfy_cli"
    assert result["diagnostics"][0]["code"] == "sugarcubes_dependency_install_failed"
    assert result["diagnostics"][0]["severity"] == "error"


def test_sync_and_check_keeps_readiness_when_default_pack_sync_fails(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Startup dependency checks should not fail hard when Base-Cubes cannot sync."""

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        _ = cwd

        class Result:
            stdout = ""

        if args == ["status", "--porcelain"]:
            Result.stdout = " M demo.cube\n"
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=tmp_path / "custom_nodes",
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _completed(command)
        ),
    )

    result = service.sync_and_check(
        {
            "sync": {"mode": "default"},
            "dependencyPolicy": {"includeVersions": True, "repair": False},
        }
    )

    assert result["syncedPacks"] == []
    assert result["syncErrors"][0]["repoRef"] == "Artificial-Sweetener/Base-Cubes"
    assert result["syncErrors"][0]["status"] == 409
    assert "local changes" in result["syncErrors"][0]["error"]
    assert result["diagnostics"][0]["code"] == "base_cubes_sync_failed"
    assert result["diagnostics"][0]["severity"] == "warning"
    assert result["dependencyReadiness"]["missingCustomNodes"] == [
        "comfyui-impact-pack"
    ]


def test_repair_preserves_failed_install_output(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Comfy CLI install failures remain visible in the repair result."""

    call_count = 0

    def runner(
        command: Sequence[str], cwd: Path, timeout_seconds: int
    ) -> subprocess.CompletedProcess[str]:
        nonlocal call_count
        _ = cwd, timeout_seconds
        call_count += 1
        return _completed(command, 0 if call_count == 1 else 1)

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=tmp_path / "custom_nodes",
        cli_adapter=ComfyCliAdapter(runner=runner),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["installedNodes"] == []
    assert result["failedNodes"][0]["nodeId"] == "comfyui-impact-pack"
    assert "stderr" in json.dumps(result["failedNodes"][0])


def test_repair_checks_out_approved_baseline_git_version(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Baseline git version repair should fetch and checkout the required commit."""

    required_commit = "37bcd403c5172adc2505b38d1d31c05969a69443"
    installed_commit = "f561f164543f927e0452e14658a0509e8e4866d6"
    git_commands: list[tuple[str, ...]] = []

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        git_commands.append(tuple(args))

        class Result:
            returncode = 0
            stdout = ""
            stderr = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = installed_commit + "\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["config", "--get", "remote.origin.url"]:
            Result.stdout = "https://github.com/Artificial-Sweetener/SimpleSyrup.git\n"
        elif args == [
            "merge-base",
            "--is-ancestor",
            required_commit,
            installed_commit,
        ]:
            Result.returncode = 1
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version=required_commit,
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    (custom_nodes_root / "SimpleSyrup" / ".git").mkdir(parents=True)
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _completed(command)
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"][0]["nodeId"] == "SimpleSyrup"
    assert result["updatedNodes"][0]["operation"] == "git_checkout"
    assert ("fetch", "--all", "--tags") in git_commands
    assert ("cat-file", "-e", f"{required_commit}^{{commit}}") in git_commands
    assert ("checkout", required_commit) in git_commands
    assert result["restartRequired"] is True


def test_repair_blocks_git_checkout_without_repository_provenance(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Version mutation must fail closed when installed repository provenance is missing."""

    required_commit = "37bcd403c5172adc2505b38d1d31c05969a69443"
    installed_commit = "f561f164543f927e0452e14658a0509e8e4866d6"

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        _ = cwd

        class Result:
            returncode = 0
            stdout = ""
            stderr = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = installed_commit + "\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == [
            "merge-base",
            "--is-ancestor",
            required_commit,
            installed_commit,
        ]:
            Result.returncode = 1
        elif args[0] == "checkout":
            raise AssertionError("checkout must not run without repository provenance")
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version=required_commit,
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    (custom_nodes_root / "SimpleSyrup" / ".git").mkdir(parents=True)
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _completed(command)
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"] == []
    assert result["failedVersionItems"][0]["reason"] == "repository_provenance_missing"


def test_repair_reports_git_runner_exception_as_failed_version_item(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Git command launch failures should stay inside the repair payload."""

    required_commit = "37bcd403c5172adc2505b38d1d31c05969a69443"
    installed_commit = "f561f164543f927e0452e14658a0509e8e4866d6"

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        _ = cwd

        class Result:
            returncode = 0
            stdout = ""
            stderr = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = installed_commit + "\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["config", "--get", "remote.origin.url"]:
            Result.stdout = "https://github.com/Artificial-Sweetener/SimpleSyrup.git\n"
        elif args == [
            "merge-base",
            "--is-ancestor",
            required_commit,
            installed_commit,
        ]:
            Result.returncode = 1
        elif args == ["fetch", "--all", "--tags"]:
            raise RuntimeError("git is unavailable")
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version=required_commit,
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    (custom_nodes_root / "SimpleSyrup" / ".git").mkdir(parents=True)
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _completed(command)
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"] == []
    assert result["failedVersionItems"][0]["nodeId"] == "SimpleSyrup"
    assert result["failedVersionItems"][0]["reason"] == "git is unavailable"
    assert (
        result["diagnostics"][0]["code"]
        == "sugarcubes_dependency_version_repair_failed"
    )


def test_repair_updates_baseline_semver_node_with_repository_provenance(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Baseline semver version repair should use Comfy CLI when provenance exists."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="ComfyUI-Impact-Pack",
            version="9.9.0",
            python_module="custom_nodes.ComfyUI-Impact-Pack",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    (custom_nodes_root / "ComfyUI-Impact-Pack").mkdir(parents=True)
    (custom_nodes_root / "ComfyUI-Impact-Pack" / ".tracking").write_text(
        json.dumps({"version": "1.0.0", "repository": "https://example.invalid/repo"}),
        encoding="utf-8",
    )
    commands: list[list[str]] = []
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _recorded_completed(
                commands, command
            ),
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"][0]["nodeId"] == "ComfyUI-Impact-Pack"
    assert result["updatedNodes"][0]["operation"] == "comfy_cli_install"
    assert commands[1][-1] == "ComfyUI-Impact-Pack"


def test_repair_skips_non_default_version_update_without_approval(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Non-default version updates should be planned but not executed silently."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    services.tracked_repos.add_repo(
        owner="Example",
        repo="Cubes",
        branch="main",
        enabled=True,
        default_base_repo=False,
    )
    checkout = services.tracked_repos.checkout_path("Example", "Cubes")
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cube_id="Example/Cubes/demo.cube",
            cnr_id="ComfyUI-Impact-Pack",
            version="9.9.0",
            python_module="custom_nodes.ComfyUI-Impact-Pack",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    (custom_nodes_root / "ComfyUI-Impact-Pack").mkdir(parents=True)
    (custom_nodes_root / "ComfyUI-Impact-Pack" / ".tracking").write_text(
        json.dumps({"version": "1.0.0", "repository": "https://example.invalid/repo"}),
        encoding="utf-8",
    )
    commands: list[list[str]] = []
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _recorded_completed(
                commands, command
            ),
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"] == []
    assert result["skippedVersionItems"][0]["nodeId"] == "ComfyUI-Impact-Pack"
    assert commands == []
