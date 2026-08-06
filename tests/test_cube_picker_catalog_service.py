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
"""Specify the native node-picker catalog projection contract."""

from __future__ import annotations

import asyncio
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.backend.responses import BackendError
from sugarcubes.backend.services.cube_picker_catalog_service import (
    CubePickerCatalogService,
)

from .conftest import FakeRequest, decode_json_response
from .typing_support import BackendServicesFactory


def test_picker_catalog_projects_canonical_boundaries_and_search_metadata() -> None:
    """Picker descriptors should exclude face controls and retain useful identity."""

    service = CubePickerCatalogService(
        list_catalog=lambda: {
            "schemaVersion": 1,
            "catalogRevision": "sha256:catalog",
            "cubes": [
                {
                    "cubeId": "Artificial-Sweetener/Base-Cubes/SDXL/Demo.cube",
                    "displayName": "SDXL/Demo",
                    "version": "1.2.3",
                    "description": "A demo Cube",
                    "targetModel": "SDXL",
                    "supportedModels": ["SDXL", "Flux"],
                    "requiredCustomNodes": ["ComfyUI-Impact-Pack"],
                    "source": {
                        "kind": "github",
                        "repoRef": "Artificial-Sweetener/Base-Cubes",
                        "path": "SDXL/Demo.cube",
                    },
                }
            ],
        },
        load_cube=lambda cube_id: {
            "cube": {
                "cube_id": cube_id,
                "version": "1.2.3",
                "default_alias": "SDXL/Demo",
                "description": "A demo Cube",
                "metadata": {
                    "tags": ["portrait", "Portrait", "detail"],
                    "author": "Asteria",
                    "surface": {
                        "controls": [{"control_id": "seed", "input_name": "seed"}]
                    },
                },
                "surface": {
                    "controls": [{"control_id": "steps", "input_name": "steps"}]
                },
            },
            "boundaries": {
                "inputs": [
                    {
                        "id": "input.image",
                        "name": "image",
                        "label": "Source image",
                        "type": "IMAGE",
                        "targets": [{"symbol": "node", "input": "image"}],
                    }
                ],
                "outputs": [
                    {
                        "id": "output.image",
                        "name": "image",
                        "label": "Detailed image",
                        "type": "IMAGE",
                        "source": {"symbol": "node", "slot": 0},
                    }
                ],
            },
        },
    )

    payload = service.list_picker_catalog()

    assert payload["schemaVersion"] == 1
    assert payload["catalogRevision"] == "sha256:catalog"
    assert payload["errors"] == []
    assert payload["entries"] == [
        {
            "key": hashlib.sha256(
                b"Artificial-Sweetener/Base-Cubes/SDXL/Demo.cube"
            ).hexdigest(),
            "cubeId": "Artificial-Sweetener/Base-Cubes/SDXL/Demo.cube",
            "version": "1.2.3",
            "displayName": "SDXL/Demo",
            "description": "A demo Cube",
            "searchTerms": [
                "Artificial-Sweetener/Base-Cubes/SDXL/Demo.cube",
                "SDXL/Demo",
                "portrait",
                "detail",
                "Asteria",
                "Artificial-Sweetener/Base-Cubes",
                "SDXL/Demo.cube",
                "SDXL",
                "Flux",
            ],
            "targetModel": "SDXL",
            "supportedModels": ["SDXL", "Flux"],
            "requiredCustomNodes": ["ComfyUI-Impact-Pack"],
            "source": {
                "kind": "github",
                "repoRef": "Artificial-Sweetener/Base-Cubes",
                "path": "SDXL/Demo.cube",
            },
            "inputs": [
                {
                    "id": "input.image",
                    "name": "image",
                    "label": "Source image",
                    "type": "IMAGE",
                }
            ],
            "outputs": [
                {
                    "id": "output.image",
                    "name": "image",
                    "label": "Detailed image",
                    "type": "IMAGE",
                }
            ],
        }
    ]
    serialized = str(payload)
    assert "control_id" not in serialized
    assert "targets" not in serialized
    assert "source': {'symbol'" not in serialized


def test_picker_catalog_omits_unloadable_cube_and_reports_error() -> None:
    """One invalid artifact should not make every valid picker entry disappear."""

    def load_cube(cube_id: str) -> dict[str, Any]:
        """Fail only the invalid catalog row."""

        if cube_id.endswith("broken.cube"):
            raise BackendError("Cube cannot be loaded", status=409)
        return {
            "cube": {"cube_id": cube_id, "version": "1.0.0"},
            "boundaries": {"inputs": [], "outputs": []},
        }

    service = CubePickerCatalogService(
        list_catalog=lambda: {
            "schemaVersion": 1,
            "catalogRevision": "sha256:mixed",
            "cubes": [
                {"cubeId": "owner/repo/good.cube", "displayName": "Good"},
                {"cubeId": "owner/repo/broken.cube", "displayName": "Broken"},
            ],
        },
        load_cube=load_cube,
    )

    payload = service.list_picker_catalog()

    assert [entry["cubeId"] for entry in payload["entries"]] == ["owner/repo/good.cube"]
    assert payload["errors"] == [
        {
            "cubeId": "owner/repo/broken.cube",
            "message": "Cube cannot be loaded",
        }
    ]


def test_picker_catalog_preserves_zero_boundaries_despite_face_controls() -> None:
    """A control-rich Cube without boundaries should advertise no node interface."""

    service = CubePickerCatalogService(
        list_catalog=lambda: {
            "schemaVersion": 1,
            "catalogRevision": "sha256:zero",
            "cubes": [{"cubeId": "owner/repo/prompt.cube", "displayName": "Prompt"}],
        },
        load_cube=lambda cube_id: {
            "cube": {
                "cube_id": cube_id,
                "surface": {
                    "controls": [
                        {"control_id": f"control-{index}"} for index in range(20)
                    ]
                },
            },
            "boundaries": {"inputs": [], "outputs": []},
        },
    )

    descriptor = service.list_picker_catalog()["entries"][0]

    assert descriptor["inputs"] == []
    assert descriptor["outputs"] == []


def test_picker_catalog_route_exposes_composed_service(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """The registered HTTP boundary should return the focused picker projection."""

    cube_id = "Artificial-Sweetener/Base-Cubes/demo.cube"
    loaded_cube = SimpleNamespace(version="1.0.0")
    prepared = SimpleNamespace(
        cube={
            "cube_id": cube_id,
            "version": "1.0.0",
            "default_alias": "Demo",
            "metadata": {"tags": ["portrait"]},
        },
        nodes=[],
        markers=[],
        connections=[],
        layout={"origin": [0, 0]},
        warnings=[],
        subgraphs=[],
        boundaries={
            "inputs": [
                {
                    "id": "seed",
                    "name": "seed",
                    "label": "Seed",
                    "type": "INT",
                    "targets": [],
                }
            ],
            "outputs": [],
        },
    )
    services = backend_services_factory(
        tmp_path,
        load_cube_artifact=lambda path: loaded_cube,
        prepare_cube_import=lambda loaded, drop_origin=(0.0, 0.0): prepared,
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    checkout.mkdir(parents=True, exist_ok=True)
    (checkout / "demo.cube").write_text(
        json.dumps(
            {
                "cube_id": cube_id,
                "version": "1.0.0",
                "metadata": {"default_alias": "Demo", "tags": ["portrait"]},
                "implementation": {
                    "nodes": {},
                    "inputs": {},
                    "outputs": {},
                    "layout": {},
                    "definitions": {},
                    "subgraphs": [],
                },
                "surface": {"default_flavor_id": "default", "controls": []},
                "flavors": {
                    "authored": [{"id": "default", "name": "Default", "values": {}}]
                },
            }
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).list_picker_catalog(FakeRequest())
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["schemaVersion"] == 1
    assert payload["catalogRevision"].startswith("sha256:")
    assert payload["entries"][0]["cubeId"] == cube_id
    assert payload["entries"][0]["inputs"] == [
        {"id": "seed", "name": "seed", "label": "Seed", "type": "INT"}
    ]
