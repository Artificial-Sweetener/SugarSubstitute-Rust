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

from __future__ import annotations

from pathlib import Path
from .typing_support import BackendServicesFactory
import asyncio
import json
from types import SimpleNamespace

from sugarcubes.backend.routes import build_route_handlers

from .conftest import FakeRequest, claim_github_owner, decode_json_response

CANONICAL_GITHUB_CUBE_ID = "Artificial-Sweetener/Base-Cubes/demo.cube"


def test_list_route_preserves_library_response_shape(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    checkout.mkdir(parents=True, exist_ok=True)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_GITHUB_CUBE_ID,
                "version": "1.0.0",
                "description": "Demo cube",
                "metadata": {
                    "author": "Alice",
                    "author_url": "https://example.com",
                    "tags": ["portrait"],
                    "supported_models": ["sdxl"],
                },
                "implementation": {
                    "nodes": {},
                    "inputs": {},
                    "outputs": {},
                    "layout": {"nodes": {"node": {}}, "markers": {}, "groups": []},
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

    response = asyncio.run(build_route_handlers(services).list_cubes(FakeRequest()))
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["count"] == 1
    assert payload["exists"] is True
    assert payload["directory"].endswith(".sugarcubes")
    assert payload["cubes"][0]["cube_id"] == CANONICAL_GITHUB_CUBE_ID
    assert payload["cubes"][0]["display_name"] == "demo"
    assert payload["cubes"][0]["target_model"] == ""
    assert payload["cubes"][0]["description"] == "Demo cube"
    assert payload["cubes"][0]["owner"] == "Artificial-Sweetener"
    assert payload["cubes"][0]["repo"] == "Base-Cubes"
    assert payload["cubes"][0]["author"] == "Artificial-Sweetener/Base-Cubes"
    assert payload["cubes"][0]["author_url"] == "https://example.com"
    assert payload["cubes"][0]["is_writable"] is False


def test_list_route_returns_route_based_default_alias(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    cube_path = checkout / "SDXL" / "Text to Image.cube"
    cube_path.parent.mkdir(parents=True, exist_ok=True)
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube",
                "version": "1.0.0",
                "metadata": {
                    "default_alias": "SDXL/Text to Image",
                    "target_model": "SDXL",
                    "supported_models": ["SD 1.5"],
                },
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

    response = asyncio.run(build_route_handlers(services).list_cubes(FakeRequest()))
    payload = decode_json_response(response)
    cube = payload["cubes"][0]

    assert response.status == 200
    assert cube["default_alias"] == "SDXL/Text to Image"
    assert cube["display_name"] == "SDXL/Text to Image"
    assert cube["target_model"] == "SDXL"
    assert cube["supported_models"] == ["SDXL", "SD 1.5"]


def test_list_route_includes_safe_icon_descriptor(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    icon_dir = checkout / "assets" / "icons"
    icon_dir.mkdir(parents=True, exist_ok=True)
    (icon_dir / "demo.png").write_bytes(b"png")
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_GITHUB_CUBE_ID,
                "version": "1.0.0",
                "metadata": {
                    "default_alias": "Demo",
                    "icon": {"kind": "asset", "path": "assets/icons/demo.png"},
                },
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

    response = asyncio.run(build_route_handlers(services).list_cubes(FakeRequest()))
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["cubes"][0]["icon"] == {
        "kind": "asset",
        "media_type": "image/png",
        "url": "/sugarcubes/assets/icon?cube_id=Artificial-Sweetener%2FBase-Cubes%2Fdemo.cube",
        "repo_relative_path": "assets/icons/demo.png",
    }
    assert str(checkout) not in json.dumps(payload["cubes"][0]["icon"])


def test_list_route_ignores_invalid_existing_icon_metadata(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    checkout.mkdir(parents=True, exist_ok=True)
    (checkout / "demo.cube").write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_GITHUB_CUBE_ID,
                "version": "1.0.0",
                "metadata": {
                    "default_alias": "Demo",
                    "icon": {
                        "kind": "asset",
                        "path": "../outside.png",
                    },
                },
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

    response = asyncio.run(build_route_handlers(services).list_cubes(FakeRequest()))
    payload = decode_json_response(response)

    assert response.status == 200
    assert "icon" not in payload["cubes"][0]


def test_icon_asset_route_serves_only_declared_cube_icon(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    icon_dir = checkout / "assets" / "icons"
    icon_dir.mkdir(parents=True, exist_ok=True)
    icon_bytes = b"\x89PNG\r\n\x1a\n"
    (icon_dir / "demo.png").write_bytes(icon_bytes)
    (icon_dir / "other.png").write_bytes(b"other")
    (checkout / "demo.cube").write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_GITHUB_CUBE_ID,
                "version": "1.0.0",
                "metadata": {
                    "default_alias": "Demo",
                    "icon": {"kind": "asset", "path": "assets/icons/demo.png"},
                },
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
        build_route_handlers(services).serve_icon_asset(
            FakeRequest(query={"cube_id": CANONICAL_GITHUB_CUBE_ID})
        )
    )

    assert response.status == 200
    assert response.content_type == "image/png"
    assert response.body == icon_bytes


def test_icon_asset_route_returns_404_for_missing_icon(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    checkout.mkdir(parents=True, exist_ok=True)
    (checkout / "demo.cube").write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_GITHUB_CUBE_ID,
                "version": "1.0.0",
                "metadata": {
                    "default_alias": "Demo",
                    "icon": {"kind": "asset", "path": "assets/icons/missing.png"},
                },
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
        build_route_handlers(services).serve_icon_asset(
            FakeRequest(query={"cube_id": CANONICAL_GITHUB_CUBE_ID})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 404
    assert "not found" in payload["error"]["message"].lower()


def test_preview_route_preserves_preview_shape(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    loaded_cube = SimpleNamespace(
        description="Preview me",
        metadata={
            "author": "Alice",
            "icon": {"kind": "asset", "path": "assets/icons/demo.svg"},
        },
        cube_id=CANONICAL_GITHUB_CUBE_ID,
        version="1.0.0",
        nodes={"n1": object(), "n2": object()},
        markers={"m1": object()},
        inputs={"IN": object()},
        outputs={"OUT": object()},
        definitions={"KSampler": {"input_order": ["seed"]}},
        warnings=["loaded warning"],
        layout=SimpleNamespace(
            groups=[{"title": "Demo"}],
            ds={"scale": 1.0, "offset": [0.0, 0.0]},
            nodes={"n1": SimpleNamespace(extra={"collapsed": True, "color": "#fff"})},
        ),
    )
    prepared = SimpleNamespace(
        nodes=[{"id": "prepared-node"}],
        markers=[{"id": "prepared-marker"}],
        connections=[{"kind": "link"}],
        warnings=["prepared warning", "loaded warning"],
        cube={},
        subgraphs=[],
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
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_GITHUB_CUBE_ID,
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
                "flavors": {
                    "authored": [{"id": "default", "name": "Default", "values": {}}]
                },
            }
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).preview_cube(
            FakeRequest(query={"cube_id": CANONICAL_GITHUB_CUBE_ID})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["cube"]["cube_id"] == CANONICAL_GITHUB_CUBE_ID
    assert payload["cube"]["default_alias"] == "demo"
    assert payload["cube"]["display_name"] == "demo"
    assert payload["cube"]["target_model"] == ""
    assert payload["cube"]["icon"] == {
        "kind": "asset",
        "media_type": "image/svg+xml",
        "url": "/sugarcubes/assets/icon?cube_id=Artificial-Sweetener%2FBase-Cubes%2Fdemo.cube",
        "repo_relative_path": "assets/icons/demo.svg",
    }
    assert payload["stats"]["nodes"] == 2
    assert payload["layout"]["collapsed_nodes"] == 1
    assert payload["warnings"] == ["loaded warning", "prepared warning"]
    assert payload["source"]["relative_path"] == "demo.cube"
    assert payload["source"]["repo_ref"] == "Artificial-Sweetener/Base-Cubes"


def test_list_route_includes_local_workspace_cubes(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    local_root = services.library.local_workspace_root() / "example-user" / "private"
    local_root.mkdir(parents=True, exist_ok=True)
    cube_path = local_root / "text_to_image.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": "local/example-user/private/text_to_image.cube",
                "version": "1.0.0",
                "description": "Local cube",
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

    response = asyncio.run(build_route_handlers(services).list_cubes(FakeRequest()))
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["count"] == 1
    assert (
        payload["cubes"][0]["cube_id"]
        == "local/example-user/private/text_to_image.cube"
    )
    assert payload["cubes"][0]["namespace"] == "example-user"
    assert payload["cubes"][0]["author"] == "local"
    assert payload["cubes"][0]["source"]["type"] == "local"
    assert payload["cubes"][0]["is_writable"] is True


def test_list_route_marks_matching_claimed_owner_repo_writable(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    checkout.mkdir(parents=True, exist_ok=True)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_GITHUB_CUBE_ID,
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
                "flavors": {
                    "authored": [{"id": "default", "name": "Default", "values": {}}]
                },
            }
        ),
        encoding="utf-8",
    )

    response = asyncio.run(build_route_handlers(services).list_cubes(FakeRequest()))
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["cubes"][0]["is_writable"] is True
    assert payload["cubes"][0]["ownership_mode"] == "mine"


def test_load_route_uses_local_source_metadata(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    loaded_cube = SimpleNamespace(version="1.0.0")
    prepared = SimpleNamespace(
        cube={
            "cube_id": "local/example-user/private/text_to_image.cube",
            "version": "1.0.0",
        },
        nodes=[],
        markers=[],
        connections=[],
        layout={"origin": [0, 0]},
        warnings=[],
        subgraphs=[],
    )
    services = backend_services_factory(
        tmp_path,
        load_cube_artifact=lambda path: loaded_cube,
        prepare_cube_import=lambda loaded, drop_origin=(0.0, 0.0): prepared,
    )
    local_root = services.library.local_workspace_root() / "example-user" / "private"
    local_root.mkdir(parents=True, exist_ok=True)
    cube_path = local_root / "text_to_image.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": "local/example-user/private/text_to_image.cube",
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
                "flavors": {
                    "authored": [{"id": "default", "name": "Default", "values": {}}]
                },
            }
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).load_cube(
            FakeRequest(
                body={"cube_id": "local/example-user/private/text_to_image.cube"}
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["source"]["type"] == "local"
    assert payload["source"]["namespace"] == "example-user"
    assert payload["source"]["relative_path"] == "private/text_to_image.cube"
    assert payload["cube"]["display_name"] == "private/text_to_image"
    assert payload["cube"]["target_model"] == "private"
