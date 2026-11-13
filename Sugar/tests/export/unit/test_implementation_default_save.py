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
"""Exercise fingerprinted implementation-default preview and commit as one flow."""

from __future__ import annotations

import asyncio
from copy import deepcopy
import json
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.exporter import ExportedCube, write_cubes_to_paths

from tests.backend_api.support.backend_fixtures import (
    FakeRequest,
    claim_github_owner,
    decode_json_response,
    ensure_tracked_repo,
)
from tests.backend_api.support.typing_support import BackendServicesFactory

CUBE_ID = "Artificial-Sweetener/Base-Cubes/demo.cube"


def _control(control_id: str, input_name: str) -> dict[str, str]:
    """Build one stable numeric surface control."""

    return {
        "control_id": control_id,
        "symbol": "size",
        "input_name": input_name,
        "label": input_name.title(),
        "class_type": "EmptyLatentImage",
        "value_type": "number",
    }


def _cube(*, width: int, height: int, version: str) -> dict[str, Any]:
    """Build one cube whose current face values are authored defaults."""

    return {
        "cube_id": CUBE_ID,
        "version": version,
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {
            "default_flavor_id": "default",
            "controls": [
                _control("size.width", "width"),
                _control("size.height", "height"),
            ],
        },
        "flavors": {
            "authored": [
                {
                    "id": "default",
                    "name": "Default",
                    "values": {"size.width": width, "size.height": height},
                }
            ]
        },
    }


def _prompt_cube(*, prompt: str, version: str) -> dict[str, Any]:
    """Build one cube with a schema-confirmed multiline prompt control."""

    payload = _cube(width=960, height=1024, version=version)
    payload["implementation"]["definitions"] = {
        "TestNode": {
            "input": {
                "required": {"prompt": ["STRING", {"default": "", "multiline": True}]}
            }
        }
    }
    payload["surface"]["controls"].append(
        {
            "control_id": "prompt.text",
            "symbol": "prompt",
            "input_name": "prompt",
            "label": "Positive prompt",
            "class_type": "TestNode",
            "value_type": "string",
        }
    )
    payload["flavors"]["authored"][0]["values"]["prompt.text"] = prompt
    return payload


def _request(*, default_review: dict[str, Any] | None = None) -> FakeRequest:
    """Build one implementation-save request with an optional reviewed decision."""

    entry: dict[str, Any] = {"cube_id": CUBE_ID, "forked": False}
    if default_review is not None:
        entry["default_review"] = default_review
    return FakeRequest(
        body={
            "graph": {"1": {"class_type": "EmptyLatentImage", "inputs": {}}},
            "workflow": {"definitions": {"subgraphs": []}},
            "workflow_version": 1,
            "actor": {"author": "Alice"},
            "cubes": [entry],
        }
    )


def test_preview_is_read_only_and_commit_applies_aggregate_default_choice(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Keep preview non-mutating and commit the reviewed structure/default pair."""

    candidate = ExportedCube(
        default_alias="Demo",
        cube=_cube(width=1080, height=1344, version="1.0.1"),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [deepcopy(candidate)],
        write_cubes_to_paths=write_cubes_to_paths,
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.0.1", reason="Implementation changed", bump="minor"
        ),
        node_class_mappings={"EmptyLatentImage": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    cube_path = ensure_tracked_repo(services) / "demo.cube"
    existing = _cube(width=960, height=1024, version="1.0.0")
    cube_path.write_text(json.dumps(existing), encoding="utf-8")
    handlers = build_route_handlers(services)

    preview_response = asyncio.run(handlers.preview_implementation(_request()))
    preview = decode_json_response(preview_response)["reviews"][0]

    assert preview_response.status == 200
    assert json.loads(cube_path.read_text(encoding="utf-8")) == existing
    assert preview["display_name"] == "demo"
    assert preview["overwrite_default_count"] == 2
    assert preview["prompt_default_count"] == 0
    assert any(
        change["label"] == "Width"
        and change["previous_value"] == 960
        and change["proposed_value"] == 1080
        for change in preview["changes"]
    )
    assert any(
        change["label"] == "Height"
        and change["previous_value"] == 1024
        and change["proposed_value"] == 1344
        for change in preview["changes"]
    )

    save_response = asyncio.run(
        handlers.save_implementation(
            _request(
                default_review={
                    "fingerprint": preview["fingerprint"],
                    "overwrite_defaults": True,
                    "save_prompt_fields": False,
                }
            )
        )
    )
    saved = json.loads(cube_path.read_text(encoding="utf-8"))

    assert save_response.status == 200
    assert saved["flavors"]["authored"][0]["values"] == {
        "size.width": 1080,
        "size.height": 1344,
    }


def test_commit_rejects_a_stale_default_review_without_writing(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Reject stale decisions before file or Git persistence."""

    candidate = ExportedCube(
        default_alias="Demo",
        cube=_cube(width=1080, height=1344, version="1.0.1"),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [deepcopy(candidate)],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"EmptyLatentImage": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    cube_path = ensure_tracked_repo(services) / "demo.cube"
    existing = _cube(width=960, height=1024, version="1.0.0")
    cube_path.write_text(json.dumps(existing), encoding="utf-8")

    response = asyncio.run(
        build_route_handlers(services).save_implementation(
            _request(
                default_review={
                    "fingerprint": "stale-fingerprint",
                    "overwrite_defaults": True,
                    "save_prompt_fields": False,
                }
            )
        )
    )

    assert response.status == 409
    assert json.loads(cube_path.read_text(encoding="utf-8")) == existing


def test_prompt_fields_are_saved_only_by_the_aggregate_prompt_choice(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Apply the prompt choice to every protected multiline field as one policy."""

    candidate = ExportedCube(
        default_alias="Demo",
        cube=_prompt_cube(prompt="authored prompt", version="1.0.1"),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [deepcopy(candidate)],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"EmptyLatentImage": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    cube_path = ensure_tracked_repo(services) / "demo.cube"
    cube_path.write_text(
        json.dumps(_prompt_cube(prompt="", version="1.0.0")), encoding="utf-8"
    )
    handlers = build_route_handlers(services)

    preview = decode_json_response(
        asyncio.run(handlers.preview_implementation(_request()))
    )["reviews"][0]
    assert preview["prompt_default_count"] == 1

    response = asyncio.run(
        handlers.save_implementation(
            _request(
                default_review={
                    "fingerprint": preview["fingerprint"],
                    "overwrite_defaults": False,
                    "save_prompt_fields": True,
                }
            )
        )
    )
    saved = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert saved["flavors"]["authored"][0]["values"]["prompt.text"] == (
        "authored prompt"
    )
