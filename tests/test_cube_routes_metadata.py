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

from typing import Any

from pathlib import Path
from .typing_support import BackendServicesFactory
import asyncio
import json
from json import JSONDecodeError

from sugarcubes.backend.routes import build_route_handlers

from .conftest import (
    FakeRequest,
    claim_github_owner,
    decode_json_response,
    ensure_tracked_repo,
)

CANONICAL_CUBE_ID = "artificial-sweetener/base-cubes/demo.cube"
OTHER_CUBE_ID = "artificial-sweetener/base-cubes/other.cube"
SDXL_CUBE_ID = "artificial-sweetener/base-cubes/SDXL/demo.cube"


def ensure_metadata_repo(services: Any) -> Any:
    """Create the tracked repo that matches the lowercase metadata test ids."""

    checkout = ensure_tracked_repo(
        services,
        owner="artificial-sweetener",
        repo="base-cubes",
        default_base_repo=False,
    )
    if not (checkout / ".git").exists():
        services.tracked_repos.git_runner(["init", "-b", "main"], cwd=checkout)
    return checkout


def commit_metadata_fixture(services: Any, checkout: Any, cube_path: Any) -> None:
    """Commit one source cube so rename tests exercise real tracked history."""

    services.tracked_repos.commit_file(
        repo_root=checkout,
        repo_relative_path=cube_path.relative_to(checkout).as_posix(),
        commit_message="create metadata fixture",
    )


def test_update_metadata_updates_description_metadata_and_version(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    cube_path = checkout / "demo.cube"
    version_layout = {
        "groups": [
            {
                "sugarcubes": {
                    "cube_id": CANONICAL_CUBE_ID,
                    "cube_version": "1.0.0",
                    "cube_definition_key": f"{CANONICAL_CUBE_ID}@1.0.0",
                }
            }
        ]
    }
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_CUBE_ID,
                "version": "1.0.0",
                "description": "before",
                "metadata": {"author": "Old", "tags": ["old"]},
                "implementation": {
                    "nodes": {},
                    "inputs": {},
                    "outputs": {},
                    "layout": version_layout,
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
    commit_metadata_fixture(services, checkout, cube_path)
    response = asyncio.run(
        build_route_handlers(services).update_metadata(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "description": "after",
                    "version": "2.0.0",
                    "metadata": {
                        "author": "Alice",
                        "author_url": "https://example.com",
                        "tags": "portrait, detail",
                        "supported_models": ["sdxl"],
                    },
                }
            )
        )
    )
    payload = decode_json_response(response)
    stored = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert payload["cube"]["version"] == "2.0.0"
    assert stored["description"] == "after"
    assert "author" not in stored["metadata"]
    assert stored["metadata"]["author_url"] == "https://example.com"
    assert stored["metadata"]["tags"] == ["portrait", "detail"]
    assert stored["metadata"]["supported_models"] == ["sdxl"]
    group_metadata = stored["implementation"]["layout"]["groups"][0]["sugarcubes"]
    assert group_metadata["cube_version"] == "2.0.0"
    assert group_metadata["cube_definition_key"] == f"{CANONICAL_CUBE_ID}@2.0.0"


def test_update_metadata_updates_default_alias_display_metadata(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_CUBE_ID,
                "version": "1.0.0",
                "metadata": {"default_alias": "demo"},
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
    commit_metadata_fixture(services, checkout, cube_path)

    response = asyncio.run(
        build_route_handlers(services).update_metadata(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "metadata": {"default_alias": "demo"},
                }
            )
        )
    )
    payload = decode_json_response(response)
    stored = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert payload["cube"]["display_name"] == "demo"
    assert stored["metadata"]["default_alias"] == "demo"


def test_update_metadata_rejects_target_model_that_conflicts_with_cube_path(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    cube_path = checkout / "SDXL" / "demo.cube"
    cube_path.parent.mkdir(parents=True, exist_ok=True)
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": SDXL_CUBE_ID,
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
        build_route_handlers(services).update_metadata(
            FakeRequest(
                body={
                    "cube_id": SDXL_CUBE_ID,
                    "metadata": {
                        "target_model": "Flux",
                        "supported_models": ["Flux"],
                    },
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 400
    assert payload["error"]["message"] == (
        "metadata.target_model must match the cube id path"
    )


def test_update_metadata_accepts_safe_icon_metadata(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_CUBE_ID,
                "version": "1.0.0",
                "metadata": {"default_alias": "Demo"},
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
        build_route_handlers(services).update_metadata(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "metadata": {
                        "icon": {
                            "kind": "asset",
                            "path": "assets/icons/demo.png",
                        },
                    },
                }
            )
        )
    )
    stored = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert stored["metadata"]["icon"] == {
        "kind": "asset",
        "path": "assets/icons/demo.png",
        "media_type": "image/png",
    }


def test_update_metadata_rejects_unsafe_icon_paths(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    (checkout / "demo.cube").write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_CUBE_ID,
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

    for unsafe_path in (
        r"E:\devprojects\SugarIcons\clean\t2i_clean.png",
        "../outside.png",
        "https://example.com/icon.png",
        "assets/icons/icon.webp",
    ):
        response = asyncio.run(
            build_route_handlers(services).update_metadata(
                FakeRequest(
                    body={
                        "cube_id": CANONICAL_CUBE_ID,
                        "metadata": {
                            "icon": {
                                "kind": "asset",
                                "path": unsafe_path,
                            },
                        },
                    }
                )
            )
        )
        payload = decode_json_response(response)

        assert response.status == 400
        assert "metadata.icon.path" in payload["error"]["message"]


def test_update_metadata_handles_cube_id_mismatch(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    class FakeRegistry:
        def __init__(self, base_dir: Any) -> None:
            self.base_dir = base_dir

        def get_path(self, cube_id: Any) -> Any:
            if cube_id == CANONICAL_CUBE_ID:
                return self.base_dir / "demo.cube"
            raise RuntimeError("missing")

    services = backend_services_factory(tmp_path, registry_factory=FakeRegistry)
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": OTHER_CUBE_ID,
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
        build_route_handlers(services).update_metadata(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "metadata": {},
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 409
    assert payload["error"]["message"] == "Cube id mismatch"
    assert payload["error"]["details"] == {
        "expected": CANONICAL_CUBE_ID,
        "actual": OTHER_CUBE_ID,
    }


def test_rename_route_preserves_response_shape(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    def retarget_payload(
        payload: Any,
        *,
        previous_cube_id: Any,
        target_cube_id: Any,
        previous_default_alias: Any,
        target_default_alias: Any,
    ) -> None:
        payload.setdefault("metadata", {})
        payload["metadata"]["default_alias"] = target_default_alias
        payload["metadata"]["previous_cube_id"] = previous_cube_id
        payload["metadata"]["previous_default_alias"] = previous_default_alias
        payload["metadata"]["retargeted_cube_id"] = target_cube_id

    services = backend_services_factory(
        tmp_path, retarget_cube_payload=retarget_payload
    )
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    cube_path = checkout / "demo.cube"
    rename_layout = {
        "groups": [
            {
                "sugarcubes": {
                    "cube_id": CANONICAL_CUBE_ID,
                    "cube_version": "1.0.0",
                    "cube_definition_key": f"{CANONICAL_CUBE_ID}@1.0.0",
                }
            }
        ]
    }
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_CUBE_ID,
                "version": "1.0.0",
                "metadata": {"default_alias": "Demo"},
                "implementation": {
                    "nodes": {},
                    "inputs": {},
                    "outputs": {},
                    "layout": rename_layout,
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
    commit_metadata_fixture(services, checkout, cube_path)
    services.local_flavors.write_cube_state(
        CANONICAL_CUBE_ID,
        {
            "cube_id": CANONICAL_CUBE_ID,
            "surfaces": {
                "surface": {
                    "flavors": [{"id": "draft", "name": "Draft", "values": {"cfg": 6}}],
                    "selected_flavor_id": "draft",
                }
            },
        },
    )

    response = asyncio.run(
        build_route_handlers(services).rename_cube(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "target_cube_id": OTHER_CUBE_ID,
                    "default_alias": "other",
                }
            )
        )
    )
    payload = decode_json_response(response)
    stored = json.loads((checkout / "other.cube").read_text(encoding="utf-8"))

    assert response.status == 200
    assert payload["cube"]["cube_id"] == OTHER_CUBE_ID
    assert payload["cube"]["name"] == "other"
    assert payload["cube"]["display_name"] == "other"
    assert payload["cube"]["version"] == "1.0.0"
    assert payload["commit"]["commit_sha"]
    assert stored["cube_id"] == OTHER_CUBE_ID
    assert stored["metadata"]["default_alias"] == "other"
    assert stored["metadata"]["previous_cube_id"] == CANONICAL_CUBE_ID
    group_metadata = stored["implementation"]["layout"]["groups"][0]["sugarcubes"]
    assert group_metadata["cube_id"] == OTHER_CUBE_ID
    assert group_metadata["cube_version"] == "1.0.0"
    assert group_metadata["cube_definition_key"] == f"{OTHER_CUBE_ID}@1.0.0"
    assert not cube_path.exists()
    assert (
        services.local_flavors.read_cube_state(OTHER_CUBE_ID)["cube_id"]
        == OTHER_CUBE_ID
    )
    assert not services.local_flavors.path_for_cube_id(CANONICAL_CUBE_ID).exists()


def test_rename_route_derives_target_from_default_alias(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    def retarget_payload(
        payload: Any,
        *,
        previous_cube_id: Any,
        target_cube_id: Any,
        previous_default_alias: Any,
        target_default_alias: Any,
    ) -> None:
        payload.setdefault("metadata", {})
        payload["metadata"]["default_alias"] = target_default_alias
        payload["metadata"]["previous_cube_id"] = previous_cube_id
        payload["metadata"]["previous_default_alias"] = previous_default_alias
        payload["metadata"]["retargeted_cube_id"] = target_cube_id

    nested_cube_id = "artificial-sweetener/base-cubes/generation/demo.cube"
    derived_cube_id = "artificial-sweetener/base-cubes/generation/Renamed Cube.cube"
    services = backend_services_factory(
        tmp_path, retarget_cube_payload=retarget_payload
    )
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    nested_dir = checkout / "generation"
    nested_dir.mkdir(parents=True, exist_ok=True)
    cube_path = nested_dir / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": nested_cube_id,
                "version": "1.0.0",
                "metadata": {"default_alias": "generation/demo"},
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
    commit_metadata_fixture(services, checkout, cube_path)

    response = asyncio.run(
        build_route_handlers(services).rename_cube(
            FakeRequest(
                body={
                    "cube_id": nested_cube_id,
                    "default_alias": "generation/Renamed Cube",
                    "derive_target_from_name": True,
                }
            )
        )
    )
    payload = decode_json_response(response)
    stored = json.loads((nested_dir / "Renamed Cube.cube").read_text(encoding="utf-8"))

    assert response.status == 200
    assert payload["cube"]["cube_id"] == derived_cube_id
    assert stored["cube_id"] == derived_cube_id
    assert stored["metadata"]["default_alias"] == "generation/Renamed Cube"
    assert stored["metadata"]["previous_cube_id"] == nested_cube_id
    assert stored["metadata"]["retargeted_cube_id"] == derived_cube_id
    assert not cube_path.exists()


def test_rename_route_moves_between_target_model_folders(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    cube_id = "artificial-sweetener/base-cubes/SDXL/demo.cube"
    target_cube_id = "artificial-sweetener/base-cubes/Flux/demo.cube"
    cube_path = checkout / "SDXL" / "demo.cube"
    cube_path.parent.mkdir(parents=True, exist_ok=True)
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": cube_id,
                "version": "1.0.0",
                "metadata": {"default_alias": "SDXL/demo", "target_model": "SDXL"},
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
    commit_metadata_fixture(services, checkout, cube_path)

    rename_response = asyncio.run(
        build_route_handlers(services).rename_cube(
            FakeRequest(
                body={
                    "cube_id": cube_id,
                    "target_cube_id": target_cube_id,
                    "default_alias": "Flux/demo",
                }
            )
        )
    )
    update_response = asyncio.run(
        build_route_handlers(services).update_metadata(
            FakeRequest(
                body={
                    "cube_id": target_cube_id,
                    "metadata": {
                        "default_alias": "Flux/demo",
                        "target_model": "Flux",
                        "supported_models": ["Flux"],
                    },
                }
            )
        )
    )
    stored = json.loads((checkout / "Flux" / "demo.cube").read_text(encoding="utf-8"))

    assert rename_response.status == 200
    assert update_response.status == 200
    assert stored["cube_id"] == target_cube_id
    assert stored["metadata"]["target_model"] == "Flux"
    assert not cube_path.exists()


def test_rename_route_derived_target_rejects_collision(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    claim_github_owner(
        services, owner="artificial-sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_metadata_repo(services)
    for cube_id, filename in (
        (CANONICAL_CUBE_ID, "demo.cube"),
        (OTHER_CUBE_ID, "other.cube"),
    ):
        (checkout / filename).write_text(
            json.dumps(
                {
                    "cube_id": cube_id,
                    "version": "1.0.0",
                    "metadata": {"default_alias": filename.removesuffix(".cube")},
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
        build_route_handlers(services).rename_cube(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "default_alias": "other",
                    "derive_target_from_name": True,
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 409
    assert (
        payload["error"]["message"]
        == "Cube 'artificial-sweetener/base-cubes/other.cube' already exists"
    )


def test_delete_route_rejects_malformed_json_body(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)

    response = asyncio.run(
        build_route_handlers(services).delete_cube(
            FakeRequest(json_error=JSONDecodeError("bad json", "", 0))
        )
    )
    payload = decode_json_response(response)

    assert response.status == 400
    assert payload["error"]["message"] == "Invalid JSON body"


def test_delete_route_preserves_missing_identifier_response(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)

    response = asyncio.run(build_route_handlers(services).delete_cube(FakeRequest()))
    payload = decode_json_response(response)

    assert response.status == 400
    assert payload["error"]["message"] == "'cube_id' is required"


def test_delete_route_blocks_read_only_tracked_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(tmp_path)
    checkout = ensure_metadata_repo(services)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            {
                "cube_id": CANONICAL_CUBE_ID,
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
        build_route_handlers(services).delete_cube(
            FakeRequest(query={"cube_id": CANONICAL_CUBE_ID})
        )
    )
    payload = decode_json_response(response)

    assert response.status == 403
    assert "read-only" in payload["error"]["message"]
    assert cube_path.exists()
