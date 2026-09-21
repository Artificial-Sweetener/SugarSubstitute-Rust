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

from tests.backend_api.support.typing_support import BackendServicesFactory

import json
import subprocess
from pathlib import Path
from typing import Sequence

import pytest

from sugarcubes.backend.services.cube_dependency_service import CubeDependencyService
from sugarcubes.backend.services.dependency_cli import ComfyCliAdapter
from sugarcubes.backend.services.dependency_python_requirements import (
    DependencyPythonRequirementsInstaller,
)
from sugarcubes.backend.services.dependency_registry_source import (
    RegistrySourceResolver,
)

from tests.library.contract.test_cube_library_backend_contract import (
    _cube_payload_with_cnr,
    _write_cube,
)


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


def _unavailable_source_resolver() -> RegistrySourceResolver:
    """Return a deterministic Registry outage boundary for failure tests."""

    def unavailable(node_id: str) -> dict[str, object]:
        """Raise the same recoverable failure as unavailable Registry metadata."""

        raise RuntimeError(f"Registry metadata unavailable for {node_id}")

    return RegistrySourceResolver(
        loader=unavailable,
        install_loader=lambda node_id, version: None,
    )


def _write_satisfied_prompt_control(custom_nodes_root: Path) -> None:
    """Materialize the exact implied dependency for unrelated repair tests."""

    installed = custom_nodes_root / "comfyui-prompt-control"
    installed.mkdir(parents=True)
    (installed / ".tracking").write_text(
        json.dumps(
            {
                "version": "3.0.0-beta.10",
                "repository": "https://github.com/asagi4/comfyui-prompt-control",
            }
        ),
        encoding="utf-8",
    )


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


def test_repair_never_removes_an_installed_pack_without_a_requirement(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Keep installed node packs when no enabled cube currently requires them."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    prompt_control = tmp_path / "custom_nodes" / "comfyui-prompt-control"
    prompt_control.mkdir(parents=True)
    marker = prompt_control / "user-owned-marker.txt"
    marker.write_text("keep", encoding="utf-8")
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=tmp_path / "custom_nodes",
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _completed(command)
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["attemptedInstallPlan"] == []
    assert result["attemptedVersionPlan"] == []
    assert marker.read_text(encoding="utf-8") == "keep"


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


def test_repair_installs_any_approved_non_default_node(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Install an approved cube requirement without a first-party allowlist."""

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
            version="4.2.0",
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

    result = service.repair(
        approval_policy="approved_node_ids",
        approved_node_ids=("comfyui-example",),
    )

    assert result["installedNodes"][0]["nodeId"] == "comfyui-example"
    assert commands[1][-1] == "comfyui-example@4.2.0"


def test_repair_reports_cli_and_registry_source_failures_without_raising(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Unavailable acquisition channels remain observable and non-fatal."""

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
        source_resolver=_unavailable_source_resolver(),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["installedNodes"] == []
    assert result["failedNodes"][0]["nodeId"] == "comfyui-impact-pack"
    assert result["failedNodes"][0]["reason"] == (
        "Registry metadata unavailable for comfyui-impact-pack"
    )
    assert result["failedNodes"][0]["registryAttempt"]["reason"] == (
        "missing_comfy_cli"
    )
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
        source_resolver=_unavailable_source_resolver(),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["installedNodes"] == []
    assert result["failedNodes"][0]["nodeId"] == "comfyui-impact-pack"
    assert "stderr" in json.dumps(result["failedNodes"][0])


def test_sync_and_check_does_not_request_restart_after_failed_repair(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Keep the running host available when approved dependency repair fails."""

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
        source_resolver=_unavailable_source_resolver(),
    )

    result = service.sync_and_check(
        {
            "dependencyPolicy": {
                "repair": True,
                "baselineOnly": True,
            }
        }
    )

    assert result["repairResult"]["failedNodes"][0]["reason"] == (
        "Registry metadata unavailable for comfyui-impact-pack"
    )
    assert result["dependencyReadiness"]["ready"] is False
    assert result["dependencyReadiness"]["restartRequired"] is True
    assert result["repairResult"]["restartRequired"] is False
    assert result["restartRequired"] is False


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


def test_repair_checks_out_exact_tag_for_clean_git_install(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Update a clean official checkout without replacing it with an archive."""

    caplog.set_level("INFO")
    installed_commit = "f561f164543f927e0452e14658a0509e8e4866d6"
    updated_commit = "0d97d7c2424f8a2d3a859fa80bfc64e935116cf1"
    custom_nodes_root = tmp_path / "custom_nodes"
    _write_satisfied_prompt_control(custom_nodes_root)
    installed_path = custom_nodes_root / "SimpleSyrup"
    git_dir = installed_path / ".git"
    git_dir.mkdir(parents=True)
    head_path = git_dir / "HEAD"
    head_path.write_text(installed_commit, encoding="utf-8")
    project_path = installed_path / "pyproject.toml"
    (installed_path / "requirements.txt").write_text("", encoding="utf-8")

    def write_project_version(version: str) -> None:
        """Write installed release evidence changed by the simulated checkout."""

        project_path.write_text(
            "[project]\n"
            'name = "SimpleSyrup"\n'
            f'version = "{version}"\n'
            "[project.urls]\n"
            'Repository = "https://github.com/Artificial-Sweetener/SimpleSyrup"\n',
            encoding="utf-8",
        )

    write_project_version("1.7.0")
    git_commands: list[tuple[str, ...]] = []

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        _ = cwd
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
        elif args == ["checkout", "v1.7.1"]:
            head_path.write_text(updated_commit, encoding="utf-8")
            write_project_version("1.7.1")
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version="1.7.1",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    _write_cube(
        checkout / "legacy-sha.cube",
        _cube_payload_with_cnr(
            cube_id="Artificial-Sweetener/Base-Cubes/legacy-sha.cube",
            cnr_id="SimpleSyrup",
            version="37bcd403c5172adc2505b38d1d31c05969a69443",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    _write_cube(
        checkout / "older-semver.cube",
        _cube_payload_with_cnr(
            cube_id="Artificial-Sweetener/Base-Cubes/older-semver.cube",
            cnr_id="SimpleSyrup",
            version="1.7.0",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    cli_commands: list[list[str]] = []
    requirement_commands: list[list[str]] = []
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        cli_adapter=ComfyCliAdapter(
            runner=lambda command, cwd, timeout_seconds: _recorded_completed(
                cli_commands, command
            )
        ),
        requirements_installer=DependencyPythonRequirementsInstaller(
            runner=lambda command, cwd, timeout_seconds: _recorded_completed(
                requirement_commands, command
            )
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"][0]["operation"] == "git_checkout"
    assert result["attemptedVersionPlan"][0]["requiredVersion"] == "1.7.1"
    assert result["attemptedVersionPlan"][0]["conflicts"] == []
    assert ("fetch", "--all", "--tags") in git_commands
    assert ("cat-file", "-e", "v1.7.1^{commit}") in git_commands
    assert ("checkout", "v1.7.1") in git_commands
    assert cli_commands == []
    assert requirement_commands[0][-2:] == [
        "-r",
        str(installed_path / "requirements.txt"),
    ]
    assert result["readinessAfter"]["ready"] is True
    checkout_count = git_commands.count(("checkout", "v1.7.1"))

    repeated = service.repair(approval_policy="silent_baseline_only")

    assert repeated["updatedNodes"] == []
    assert repeated["restartRequired"] is False
    assert repeated["readinessAfter"]["ready"] is True
    assert git_commands.count(("checkout", "v1.7.1")) == checkout_count
    progress = [
        message for message in caplog.messages if "SugarCubes[nodepack_" in message
    ]
    assert [message.split("]", 1)[0] for message in progress] == [
        "SugarCubes[nodepack_version_selected",
        "SugarCubes[nodepack_source_selected",
        "SugarCubes[nodepack_update_started",
        "SugarCubes[nodepack_update_complete",
        "SugarCubes[nodepack_restart_required",
    ]


def test_repair_checks_out_required_tag_for_any_clean_git_install(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Use an installed pack's own clean Git origin without an allowlist."""

    node_id = "ExampleThirdPartyPack"
    installed_commit = "1111111111111111111111111111111111111111"
    updated_commit = "2222222222222222222222222222222222222222"
    repository_url = "https://github.com/example/third-party-pack.git"
    custom_nodes_root = tmp_path / "custom_nodes"
    installed_path = custom_nodes_root / node_id
    git_dir = installed_path / ".git"
    git_dir.mkdir(parents=True)
    head_path = git_dir / "HEAD"
    head_path.write_text(installed_commit, encoding="utf-8")
    project_path = installed_path / "pyproject.toml"
    requirements_path = installed_path / "requirements.txt"
    requirements_path.write_text("", encoding="utf-8")

    def write_project_version(version: str) -> None:
        """Write version evidence changed by the simulated tag checkout."""

        project_path.write_text(
            "[project]\n"
            f'name = "{node_id}"\n'
            f'version = "{version}"\n'
            "[project.urls]\n"
            f'Repository = "{repository_url}"\n',
            encoding="utf-8",
        )

    write_project_version("1.0.0")
    git_commands: list[tuple[str, ...]] = []

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        class Result:
            returncode = 0
            stdout = ""
            stderr = ""

        if cwd != installed_path:
            return Result()
        command = tuple(args)
        git_commands.append(command)
        if args == ["rev-parse", "HEAD"]:
            Result.stdout = head_path.read_text(encoding="utf-8") + "\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["config", "--get", "remote.origin.url"]:
            Result.stdout = repository_url + "\n"
        elif args == ["checkout", "v2.0.0"]:
            head_path.write_text(updated_commit, encoding="utf-8")
            write_project_version("2.0.0")
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "third-party.cube",
        _cube_payload_with_cnr(
            cnr_id=node_id,
            version="2.0.0",
            python_module=f"custom_nodes.{node_id}",
        ),
    )
    requirement_commands: list[list[str]] = []

    def reject_registry(command: Sequence[str], cwd: Path, timeout_seconds: int) -> Any:
        """Fail if a clean Git installation is sent through Registry repair."""

        _ = command, cwd, timeout_seconds
        raise AssertionError("a clean Git install must use its own origin")

    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        cli_adapter=ComfyCliAdapter(runner=reject_registry),
        requirements_installer=DependencyPythonRequirementsInstaller(
            runner=lambda command, cwd, timeout_seconds: _recorded_completed(
                requirement_commands, command
            )
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"][0]["nodeId"] == node_id
    assert result["updatedNodes"][0]["operation"] == "git_checkout"
    assert ("fetch", "--all", "--tags") in git_commands
    assert ("cat-file", "-e", "v2.0.0^{commit}") in git_commands
    assert ("checkout", "v2.0.0") in git_commands
    assert requirement_commands[0][-2:] == ["-r", str(requirements_path)]
    assert result["readinessAfter"]["ready"] is True


def test_failed_requirements_restore_the_previous_git_revision(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Roll back source changes when post-checkout requirements fail."""

    installed_commit = "f561f164543f927e0452e14658a0509e8e4866d6"
    updated_commit = "0d97d7c2424f8a2d3a859fa80bfc64e935116cf1"
    custom_nodes_root = tmp_path / "custom_nodes"
    installed_path = custom_nodes_root / "SimpleSyrup"
    git_dir = installed_path / ".git"
    git_dir.mkdir(parents=True)
    head_path = git_dir / "HEAD"
    project_path = installed_path / "pyproject.toml"
    (installed_path / "requirements.txt").write_text("", encoding="utf-8")

    def write_installed_state(commit: str, version: str) -> None:
        """Write the source evidence changed by the simulated checkout."""

        head_path.write_text(commit, encoding="utf-8")
        project_path.write_text(
            "[project]\n"
            'name = "SimpleSyrup"\n'
            f'version = "{version}"\n'
            "[project.urls]\n"
            'Repository = "https://github.com/Artificial-Sweetener/SimpleSyrup"\n',
            encoding="utf-8",
        )

    write_installed_state(installed_commit, "1.7.0")
    git_commands: list[tuple[str, ...]] = []

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        _ = cwd
        command = tuple(args)
        git_commands.append(command)

        class Result:
            returncode = 0
            stdout = ""
            stderr = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = head_path.read_text(encoding="utf-8") + "\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["config", "--get", "remote.origin.url"]:
            Result.stdout = "https://github.com/Artificial-Sweetener/SimpleSyrup.git\n"
        elif args == ["checkout", "v1.7.1"]:
            write_installed_state(updated_commit, "1.7.1")
        elif args == ["checkout", installed_commit]:
            write_installed_state(installed_commit, "1.7.0")
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version="1.7.1",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        requirements_installer=DependencyPythonRequirementsInstaller(
            runner=lambda command, cwd, timeout_seconds: _completed(command, 1)
        ),
    )

    result = service.repair(approval_policy="silent_baseline_only")
    failure = result["failedVersionItems"][0]

    assert result["updatedNodes"] == []
    assert result["restartRequired"] is False
    assert "Could not install SimpleSyrup requirements" in failure["reason"]
    assert failure["rollbackRef"] == installed_commit
    assert failure["rollbackSucceeded"] is True
    assert ("checkout", "v1.7.1") in git_commands
    assert ("checkout", installed_commit) in git_commands
    assert 'version = "1.7.0"' in project_path.read_text(encoding="utf-8")


def test_repair_refuses_dirty_first_party_git_semver_update(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Never mutate a first-party checkout with local changes."""

    caplog.set_level("INFO")

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        _ = cwd

        class Result:
            returncode = 0
            stdout = ""
            stderr = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = "f561f164543f927e0452e14658a0509e8e4866d6\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = " M local.py\n"
        elif args == ["config", "--get", "remote.origin.url"]:
            Result.stdout = "https://github.com/Artificial-Sweetener/SimpleSyrup.git\n"
        elif args[0] in {"fetch", "checkout"}:
            raise AssertionError("dirty checkout must not be changed")
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version="1.7.1",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    _write_satisfied_prompt_control(custom_nodes_root)
    (custom_nodes_root / "SimpleSyrup" / ".git").mkdir(parents=True)
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
        cli_adapter=ComfyCliAdapter(),
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"] == []
    assert result["failedVersionItems"] == []
    assert result["blockedVersionItems"][0]["status"] == "blocked"
    assert result["blockedVersionItems"][0]["repairable"] is False
    assert result["diagnostics"][0]["code"] == (
        "sugarcubes_dependency_version_repair_blocked"
    )
    progress = [
        message for message in caplog.messages if "SugarCubes[nodepack_" in message
    ]
    assert sum("nodepack_update_failed" in message for message in progress) == 1
    assert not any("nodepack_update_complete" in message for message in progress)
    assert progress[-1].endswith("ComfyUI will continue starting.")


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


def test_semver_git_repair_uses_the_actual_remote_as_authoritative_source(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Follow a clean checkout's actual origin instead of project URL metadata."""

    installed_commit = "f561f164543f927e0452e14658a0509e8e4866d6"
    checkout_commands: list[tuple[str, ...]] = []
    project_path: Path | None = None

    def fake_git(args: Sequence[str], *, cwd: Path) -> Any:
        class Result:
            returncode = 0
            stdout = ""
            stderr = ""

        if args == ["rev-parse", "HEAD"]:
            Result.stdout = installed_commit + "\n"
        elif args == ["status", "--porcelain"]:
            Result.stdout = ""
        elif args == ["config", "--get", "remote.origin.url"]:
            Result.stdout = "https://example.invalid/not-simple-syrup.git\n"
        elif args[0] == "checkout":
            checkout_commands.append(tuple(args))
            assert project_path is not None
            project_path.write_text(
                "[project]\n"
                'name = "SimpleSyrup"\n'
                'version = "1.7.1"\n'
                "[project.urls]\n"
                'Repository = "https://example.invalid/not-simple-syrup.git"\n',
                encoding="utf-8",
            )
        return Result()

    services = backend_services_factory(tmp_path, git_runner=fake_git)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(
        checkout / "demo.cube",
        _cube_payload_with_cnr(
            cnr_id="SimpleSyrup",
            version="1.7.1",
            python_module="custom_nodes.SimpleSyrup",
        ),
    )
    custom_nodes_root = tmp_path / "custom_nodes"
    _write_satisfied_prompt_control(custom_nodes_root)
    installed = custom_nodes_root / "SimpleSyrup"
    (installed / ".git").mkdir(parents=True)
    project_path = installed / "pyproject.toml"
    project_path.write_text(
        "[project]\n"
        'name = "SimpleSyrup"\n'
        'version = "1.7.0"\n'
        "[project.urls]\n"
        'Repository = "https://github.com/Artificial-Sweetener/SimpleSyrup"\n',
        encoding="utf-8",
    )
    service = CubeDependencyService(
        library_service=services.library,
        tracked_repo_service=services.tracked_repos,
        workspace_path=tmp_path / "ComfyUI",
        custom_nodes_root=custom_nodes_root,
    )

    result = service.repair(approval_policy="silent_baseline_only")

    assert result["updatedNodes"][0]["operation"] == "git_checkout"
    assert checkout_commands == [("checkout", "v1.7.1")]
    assert result["failedVersionItems"] == []
    assert result["restartRequired"] is True
    assert result["readinessAfter"]["ready"] is True


def test_repair_reports_git_runner_exception_as_failed_version_item(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Git command launch failures should stay inside the repair payload."""

    caplog.set_level("INFO")

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
    failures = [
        message
        for message in caplog.messages
        if "SugarCubes[nodepack_update_failed]" in message
    ]
    assert len(failures) == 1
    assert "git is unavailable" in failures[0]
    assert failures[0].endswith("ComfyUI will continue starting.")


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
    assert result["updatedNodes"][0]["operation"] == "comfy_registry_install"
    assert commands[1][-1] == "ComfyUI-Impact-Pack@9.9.0"


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
