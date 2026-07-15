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
"""Route tests for SugarCubes dependency maintenance."""

from __future__ import annotations

from .typing_support import BackendServicesFactory

import asyncio
import subprocess
from pathlib import Path
from typing import Sequence

from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.backend.services.cube_dependency_service import (
    ComfyCliAdapter,
    CubeDependencyService,
)

from .conftest import FakeRequest, decode_json_response
from .test_cube_library_backend_contract import _cube_payload_with_cnr, _write_cube


def test_dependency_readiness_route_has_no_install_side_effect(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Readiness route returns the plan without invoking Comfy CLI."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())

    response = asyncio.run(
        build_route_handlers(services).get_dependency_readiness(FakeRequest())
    )
    payload = decode_json_response(response)

    assert payload["missingCustomNodes"] == ["comfyui-impact-pack"]
    assert payload["installPlan"][0]["confirmationRequired"] is False


def test_dependency_repair_route_installs_approved_nodes(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Repair route forwards approved node ids into the dependency service."""

    commands: list[list[str]] = []

    def runner(
        command: Sequence[str], cwd: Path, timeout_seconds: int
    ) -> subprocess.CompletedProcess[str]:
        _ = cwd, timeout_seconds
        commands.append(list(command))
        return subprocess.CompletedProcess(list(command), 0, stdout="ok", stderr="")

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
    object.__setattr__(
        services,
        "dependencies",
        CubeDependencyService(
            library_service=services.library,
            tracked_repo_service=services.tracked_repos,
            workspace_path=tmp_path / "ComfyUI",
            custom_nodes_root=tmp_path / "custom_nodes",
            cli_adapter=ComfyCliAdapter(runner=runner),
        ),
    )

    response = asyncio.run(
        build_route_handlers(services).repair_dependencies(
            FakeRequest(body={"approvedNodeIds": ["comfyui-example"]})
        )
    )
    payload = decode_json_response(response)

    assert payload["attemptedInstallPlan"][0]["nodeId"] == "comfyui-example"
    assert commands[1][-1] == "comfyui-example"


def test_dependency_sync_and_check_route_returns_readiness_plan(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Shared orchestration route recomputes readiness after the requested sync."""

    services = backend_services_factory(tmp_path, git_runner=lambda args, cwd: None)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _write_cube(checkout / "demo.cube", _cube_payload_with_cnr())

    response = asyncio.run(
        build_route_handlers(services).sync_and_check_dependencies(
            FakeRequest(
                body={
                    "sync": {"mode": ""},
                    "dependencyPolicy": {"includeVersions": True, "repair": False},
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert payload["schemaVersion"] == 1
    assert payload["syncedPacks"] == []
    assert payload["dependencyReadiness"]["missingCustomNodes"] == [
        "comfyui-impact-pack"
    ]
    assert payload["repairPlan"]["installPlan"][0]["nodeId"] == "comfyui-impact-pack"
