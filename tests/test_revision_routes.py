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
"""Revision route and service tests for git-backed SugarCubes history."""

from __future__ import annotations

from typing import Any

from pathlib import Path
from .typing_support import BackendServicesFactory

import asyncio
import json
from types import SimpleNamespace

from sugarcubes.backend.routes import build_route_handlers

from .conftest import FakeRequest, decode_json_response


def _make_loaded_cube(path: Any) -> Any:
    payload = json.loads(path.read_text(encoding="utf-8"))
    return SimpleNamespace(
        description=payload.get("description", ""),
        metadata=payload.get("metadata", {}),
        cube_id=payload.get("cube_id", ""),
        version=payload.get("version", ""),
        nodes={},
        markers={},
        inputs={},
        outputs={},
        definitions={},
        warnings=[],
        layout=None,
    )


def _make_prepared_import(loaded: Any, drop_origin: Any = (0.0, 0.0)) -> Any:
    return SimpleNamespace(
        cube={
            "cube_id": loaded.cube_id,
            "version": loaded.version,
            "metadata": dict(loaded.metadata),
        },
        nodes=[],
        markers=[],
        connections=[],
        layout={"origin": list(drop_origin)},
        warnings=[],
        subgraphs=[],
    )


def test_list_revisions_route_returns_current_and_commits_for_github_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    historical_payload = {
        "cube_id": "Artificial-Sweetener/Base-Cubes/demo.cube",
        "version": "1.0.0",
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }
    current_payload = {
        **historical_payload,
        "version": "1.0.1",
    }

    def fake_git(args: Any, *, cwd: Any) -> Any:
        command = list(args)
        if command[0] == "log":
            return SimpleNamespace(
                stdout="abc123456789\x1f2026-04-06T17:00:00+00:00\x1fInitial version\n"
            )
        if command[0] == "show":
            return SimpleNamespace(stdout=json.dumps(historical_payload))
        raise AssertionError(f"Unexpected git command: {command}")

    services = backend_services_factory(
        tmp_path,
        git_runner=fake_git,
        load_cube_artifact=_make_loaded_cube,
        prepare_cube_import=_make_prepared_import,
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener",
        "Base-Cubes",
    )
    checkout.mkdir(parents=True, exist_ok=True)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(json.dumps(current_payload), encoding="utf-8")

    response = asyncio.run(
        build_route_handlers(services).list_revisions(
            FakeRequest(query={"cube_id": "Artificial-Sweetener/Base-Cubes/demo.cube"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["count"] == 2
    assert payload["revisions"][0]["revision_ref"] == "WORKTREE"
    assert payload["revisions"][0]["version"] == "1.0.1"
    assert payload["revisions"][1]["revision_ref"] == "abc123456789"
    assert payload["revisions"][1]["version"] == "1.0.0"


def test_list_revisions_route_keeps_same_version_commits(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    first_historical_payload = {
        "cube_id": "Artificial-Sweetener/Base-Cubes/demo.cube",
        "version": "1.0.0",
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }
    second_historical_payload = {
        **first_historical_payload,
        "description": "same version",
    }
    current_payload = {**first_historical_payload, "version": "1.1.0"}

    def fake_git(args: Any, *, cwd: Any) -> Any:
        command = list(args)
        if command[0] == "log":
            return SimpleNamespace(
                stdout=(
                    "def123456789\x1f2026-04-07T17:00:00+00:00\x1fDuplicate version\n"
                    "abc123456789\x1f2026-04-06T17:00:00+00:00\x1fOriginal version\n"
                )
            )
        if command[0] == "show":
            revision_ref = command[1].split(":", 1)[0]
            payload = (
                second_historical_payload
                if revision_ref == "def123456789"
                else first_historical_payload
            )
            return SimpleNamespace(stdout=json.dumps(payload))
        raise AssertionError(f"Unexpected git command: {command}")

    services = backend_services_factory(
        tmp_path,
        git_runner=fake_git,
        load_cube_artifact=_make_loaded_cube,
        prepare_cube_import=_make_prepared_import,
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener",
        "Base-Cubes",
    )
    checkout.mkdir(parents=True, exist_ok=True)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(json.dumps(current_payload), encoding="utf-8")

    response = asyncio.run(
        build_route_handlers(services).list_revisions(
            FakeRequest(query={"cube_id": "Artificial-Sweetener/Base-Cubes/demo.cube"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["count"] == 3
    assert [entry["version"] for entry in payload["revisions"]] == [
        "1.1.0",
        "1.0.0",
        "1.0.0",
    ]
    assert [entry["revision_ref"] for entry in payload["revisions"]] == [
        "WORKTREE",
        "def123456789",
        "abc123456789",
    ]
    assert payload["duplicate_version_omissions"] == []


def test_list_revisions_route_returns_current_for_uncommitted_local_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    current_payload = {
        "cube_id": "local/example-user/private/demo.cube",
        "version": "0.1.0",
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }

    def fake_git(args: Any, *, cwd: Any) -> Any:
        command = list(args)
        if command[:3] == ["init", "-b", "main"]:
            return SimpleNamespace(stdout="")
        if command[0] == "log":
            raise RuntimeError(
                "your current branch 'main' does not have any commits yet"
            )
        raise AssertionError(f"Unexpected git command: {command}")

    services = backend_services_factory(
        tmp_path,
        git_runner=fake_git,
        load_cube_artifact=_make_loaded_cube,
        prepare_cube_import=_make_prepared_import,
    )
    local_root = services.library.local_workspace_root() / "example-user" / "private"
    local_root.mkdir(parents=True, exist_ok=True)
    cube_path = local_root / "demo.cube"
    cube_path.write_text(json.dumps(current_payload), encoding="utf-8")
    (services.library.local_workspace_root() / ".git").mkdir(exist_ok=True)

    response = asyncio.run(
        build_route_handlers(services).list_revisions(
            FakeRequest(query={"cube_id": "local/example-user/private/demo.cube"})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["count"] == 1
    assert payload["revisions"][0]["revision_ref"] == "WORKTREE"
    assert payload["revisions"][0]["version"] == "0.1.0"


def test_load_revision_route_returns_historical_prepared_import(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    historical_payload = {
        "cube_id": "Artificial-Sweetener/Base-Cubes/SDXL/Demo.cube",
        "version": "1.0.0",
        "metadata": {"default_alias": "SDXL/Demo", "target_model": "SDXL"},
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }
    current_payload = {
        **historical_payload,
        "version": "1.0.2",
    }

    def fake_git(args: Any, *, cwd: Any) -> Any:
        command = list(args)
        if command[0] == "show":
            return SimpleNamespace(stdout=json.dumps(historical_payload))
        if command[0] == "log":
            return SimpleNamespace(stdout="")
        raise AssertionError(f"Unexpected git command: {command}")

    services = backend_services_factory(
        tmp_path,
        git_runner=fake_git,
        load_cube_artifact=_make_loaded_cube,
        prepare_cube_import=_make_prepared_import,
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener",
        "Base-Cubes",
    )
    checkout.mkdir(parents=True, exist_ok=True)
    cube_path = checkout / "SDXL" / "Demo.cube"
    cube_path.parent.mkdir(parents=True, exist_ok=True)
    cube_path.write_text(json.dumps(current_payload), encoding="utf-8")

    response = asyncio.run(
        build_route_handlers(services).load_revision(
            FakeRequest(
                body={
                    "cube_id": "Artificial-Sweetener/Base-Cubes/SDXL/Demo.cube",
                    "revision_ref": "abc123456789",
                    "origin": {"x": 10, "y": 20},
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["cube"]["version"] == "1.0.0"
    assert payload["cube"]["default_alias"] == "SDXL/Demo"
    assert payload["cube"]["display_name"] == "SDXL/Demo"
    assert payload["cube"]["target_model"] == "SDXL"
    assert payload["revision"]["revision_ref"] == "abc123456789"
    assert payload["revision"]["current"] is False
