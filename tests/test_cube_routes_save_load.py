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
"""Save/load route tests for the tracked-repo cube model."""

from __future__ import annotations

from typing import Any

from pathlib import Path
from .typing_support import BackendServicesFactory

import asyncio
import json
import logging
from types import SimpleNamespace

import pytest

from sugarcubes.importer import CubeImportError
from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.exporter import ExportedCube, write_cube_to_path, write_cubes_to_paths

from .conftest import (
    FakeRequest,
    claim_github_owner,
    decode_json_response,
    ensure_tracked_repo,
)

CANONICAL_CUBE_ID = "Artificial-Sweetener/Base-Cubes/demo.cube"


def _write_current_cube(
    path: Any,
    *,
    cube_id: Any = CANONICAL_CUBE_ID,
    version: Any = "1.0.0",
    metadata: Any = None,
) -> None:
    payload = {
        "cube_id": cube_id,
        "version": version,
        "metadata": dict(metadata or {}),
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
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload), encoding="utf-8")


def _surface_control(
    control_id: Any,
    *,
    input_name: Any,
    label: Any = None,
    class_type: Any = "KSampler",
    symbol: Any = "ksampler",
    value_type: Any = "number",
) -> Any:
    """Build one compact numeric surface-control fixture."""

    return {
        "control_id": control_id,
        "symbol": symbol,
        "input_name": input_name,
        "label": label or input_name,
        "class_type": class_type,
        "value_type": value_type,
    }


def _surface_cube_payload(
    *,
    cube_id: Any = CANONICAL_CUBE_ID,
    version: Any = "1.0.0",
    description: Any = "",
    metadata: Any = None,
    controls: Any = None,
    authored: Any = None,
) -> Any:
    """Build one current-format cube fixture with surface controls."""

    return {
        "cube_id": cube_id,
        "version": version,
        "description": description,
        "metadata": dict(metadata or {}),
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
            "controls": list(
                controls
                if controls is not None
                else [_surface_control("ksampler.cfg", input_name="cfg")]
            ),
        },
        "flavors": {
            "authored": list(
                authored
                if authored is not None
                else [
                    {
                        "id": "default",
                        "name": "Default",
                        "values": {"ksampler.cfg": 7},
                    }
                ]
            )
        },
    }


def _init_git_repo(services: Any, repo_root: Any) -> None:
    """Initialize one isolated git repo for backend save tests."""

    repo_root.mkdir(parents=True, exist_ok=True)
    services.tracked_repos.git_runner(["init", "-b", "main"], cwd=repo_root)


def _commit_all(services: Any, repo_root: Any, message: Any = "baseline") -> None:
    """Commit all current fixture files in one isolated git repo."""

    services.tracked_repos.git_runner(["add", "--all"], cwd=repo_root)
    services.tracked_repos.git_runner(
        [
            "-c",
            "user.name=SugarCubes",
            "-c",
            "user.email=sugarcubes@example.invalid",
            "commit",
            "-m",
            message,
        ],
        cwd=repo_root,
    )


def test_save_many_persists_to_tracked_repo_checkout(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(version="1.0.0"),
        warnings=["warning one"],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=lambda cube_targets, overwrite=True: [
            {
                "path": str(target_path),
                "filename": target_path.name,
                "default_alias": exported.default_alias,
            }
            for _, target_path in cube_targets
        ],
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.0.1", reason="patch", bump="patch"
        ),
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    _init_git_repo(services, checkout)
    _write_current_cube(checkout / "demo.cube", metadata={"author": "Alice"})
    _commit_all(services, checkout)

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice", "author_url": "https://example.com"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["saved"][0]["cube_id"] == CANONICAL_CUBE_ID
    assert payload["saved"][0]["path"].endswith("demo.cube")
    assert payload["warnings"] == ["Demo: warning one"]
    assert payload["version_suggestions"][0]["suggested_version"] == "1.0.1"
    assert "author" not in exported.cube["metadata"]
    assert exported.cube["metadata"]["author_url"] == "https://example.com"


def test_save_many_accepts_omitted_actor_and_preserves_existing_author_url(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Keep personal saves zero-setup without erasing previously authored links."""

    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            version="1.0.0",
            metadata={"author_url": "https://example.com/existing"},
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=lambda cube_targets, overwrite=True: [
            {
                "path": str(target_path),
                "filename": target_path.name,
                "default_alias": exported.default_alias,
            }
            for _, target_path in cube_targets
        ],
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    _init_git_repo(services, checkout)
    _write_current_cube(
        checkout / "demo.cube",
        metadata={"author_url": "https://example.com/existing"},
    )
    _commit_all(services, checkout)

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )

    assert response.status == 200
    assert exported.cube["metadata"]["author_url"] == "https://example.com/existing"


def test_save_many_applies_explicit_description_override(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            version="1.0.0",
            description="Auto-converted cube for demo",
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=lambda cube_targets, overwrite=True: [
            {
                "path": str(target_path),
                "filename": target_path.name,
                "default_alias": exported.default_alias,
            }
            for _, target_path in cube_targets
        ],
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    _write_current_cube(checkout / "demo.cube", metadata={"author": "Alice"})

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [
                        {
                            "cube_id": CANONICAL_CUBE_ID,
                            "forked": False,
                            "description": "",
                        }
                    ],
                }
            )
        )
    )

    assert response.status == 200
    assert exported.cube["description"] == ""


def test_save_many_applies_entry_target_model_metadata(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    cube_id = "Artificial-Sweetener/Base-Cubes/SDXL/Demo.cube"
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(cube_id=cube_id, version="1.0.0", metadata={}),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=lambda cube_targets, overwrite=True: [
            {
                "path": str(target_path),
                "filename": target_path.name,
                "default_alias": exported.default_alias,
            }
            for _, target_path in cube_targets
        ],
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    ensure_tracked_repo(services)

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [
                        {
                            "cube_id": cube_id,
                            "forked": False,
                            "metadata": {
                                "target_model": "SDXL",
                                "supported_models": ["SD 1.5"],
                            },
                        }
                    ],
                }
            )
        )
    )

    assert response.status == 200
    assert exported.cube["metadata"]["default_alias"] == "SDXL/Demo"
    assert exported.cube["metadata"]["target_model"] == "SDXL"
    assert exported.cube["metadata"]["supported_models"] == ["SDXL", "SD 1.5"]


def test_save_many_preserves_catalog_metadata_on_implementation_save(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            version="1.0.0",
            description="Auto-converted cube for demo",
            metadata={"author_url": "https://actor.example"},
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=lambda cube_targets, overwrite=True: [
            {
                "path": str(target_path),
                "filename": target_path.name,
                "default_alias": exported.default_alias,
            }
            for _, target_path in cube_targets
        ],
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    _write_current_cube(
        checkout / "demo.cube",
        metadata={
            "default_alias": "demo",
            "author_url": "https://original.example",
        },
    )
    payload = json.loads((checkout / "demo.cube").read_text(encoding="utf-8"))
    payload["description"] = "Edited description"
    (checkout / "demo.cube").write_text(json.dumps(payload), encoding="utf-8")

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice", "author_url": "https://actor.example"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )

    assert response.status == 200
    assert exported.cube["description"] == "Edited description"
    assert exported.cube["metadata"]["default_alias"] == "demo"
    assert exported.cube["metadata"]["author_url"] == "https://actor.example"


def test_save_many_repairs_missing_default_alias_on_implementation_save(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    exported = ExportedCube(
        default_alias="Image to Image",
        cube=_surface_cube_payload(
            cube_id="Artificial-Sweetener/Base-Cubes/Image to Image.cube",
            version="1.0.0",
            description="Auto-converted cube for image to image",
            metadata={"author_url": "https://actor.example"},
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=lambda cube_targets, overwrite=True: [
            {
                "path": str(target_path),
                "filename": target_path.name,
                "default_alias": exported.default_alias,
            }
            for _, target_path in cube_targets
        ],
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    _write_current_cube(
        checkout / "Image to Image.cube",
        cube_id="Artificial-Sweetener/Base-Cubes/Image to Image.cube",
        metadata={"author_url": "https://original.example"},
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice", "author_url": "https://actor.example"},
                    "cubes": [
                        {
                            "cube_id": "Artificial-Sweetener/Base-Cubes/Image to Image.cube",
                            "forked": False,
                        }
                    ],
                }
            )
        )
    )

    assert response.status == 200
    assert exported.cube["metadata"]["default_alias"] == "Image to Image"
    assert exported.cube["metadata"]["author_url"] == "https://actor.example"


def test_save_many_commits_version_changed_github_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(version="1.0.1"),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _init_git_repo(services, checkout)
    _write_current_cube(
        checkout / "demo.cube", version="1.0.0", metadata={"author": "Alice"}
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)
    saved_entry = payload["saved"][0]

    assert response.status == 200
    assert saved_entry["committed"] is True
    assert saved_entry["commit_sha"]
    assert saved_entry["commit_short_sha"]
    assert saved_entry["commit_message"] == "update demo.cube v1.0.1"
    revisions = services.revisions.list_revisions(cube_id=CANONICAL_CUBE_ID)
    assert revisions["count"] == 1
    assert revisions["revisions"][0]["version"] == "1.0.1"


def test_save_many_stale_latest_uses_current_version_suggestion(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    seen_versions = {}
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(version="1.0.1"),
        warnings=[],
        version_auto=False,
    )
    exported.cube["implementation"]["layout"] = {
        "groups": [
            {
                "sugarcubes": {
                    "cube_id": CANONICAL_CUBE_ID,
                    "cube_version": "1.0.1",
                    "cube_definition_key": f"{CANONICAL_CUBE_ID}@1.0.1",
                }
            }
        ]
    }

    def suggest_version(existing: Any, current: Any) -> Any:
        seen_versions["existing"] = existing.get("version")
        seen_versions["current"] = current.get("version")
        return SimpleNamespace(suggested="1.2.2", reason="patch", bump="patch")

    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        suggest_version=suggest_version,
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _init_git_repo(services, checkout)
    _write_current_cube(
        checkout / "demo.cube", version="1.2.1", metadata={"author": "Alice"}
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [
                        {
                            "cube_id": CANONICAL_CUBE_ID,
                            "forked": False,
                            "source_revision_ref": "abc123456789",
                            "source_version": "1.0.1",
                            "source_definition_key": (f"{CANONICAL_CUBE_ID}@1.0.1"),
                            "stale_save_mode": "latest",
                        }
                    ],
                }
            )
        )
    )
    payload = decode_json_response(response)
    saved_entry = payload["saved"][0]
    saved_payload = json.loads((checkout / "demo.cube").read_text(encoding="utf-8"))

    assert response.status == 200
    assert seen_versions == {"existing": "1.2.1", "current": "1.0.1"}
    assert "version_suggestions" not in payload
    assert exported.cube["version"] == "1.2.2"
    assert saved_payload["version"] == "1.2.2"
    group_metadata = saved_payload["implementation"]["layout"]["groups"][0][
        "sugarcubes"
    ]
    assert group_metadata["cube_version"] == "1.2.2"
    assert group_metadata["cube_definition_key"] == f"{CANONICAL_CUBE_ID}@1.2.2"
    assert saved_entry["version"] == "1.2.2"
    assert saved_entry["committed"] is True
    assert saved_entry["commit_message"] == "update demo.cube v1.2.2"


def test_save_many_existing_implementation_save_preserves_authored_flavors(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    existing_controls = [
        _surface_control("ksampler.cfg", input_name="cfg"),
        _surface_control("ksampler.steps", input_name="steps"),
    ]
    exported_controls = [
        _surface_control("ksampler.cfg", input_name="cfg"),
        _surface_control("ksampler.seed", input_name="seed"),
    ]
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            version="1.0.1",
            controls=exported_controls,
            authored=[
                {
                    "id": "default",
                    "name": "Default",
                    "values": {"ksampler.cfg": 99, "ksampler.seed": 12345},
                }
            ],
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.0.1", reason="Implementation changed", bump="minor"
        ),
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            _surface_cube_payload(
                version="1.0.0",
                controls=existing_controls,
                authored=[
                    {
                        "id": "default",
                        "name": "Default",
                        "values": {"ksampler.cfg": 7, "ksampler.steps": 30},
                    },
                    {
                        "id": "portrait",
                        "name": "Portrait",
                        "values": {"ksampler.cfg": 8, "ksampler.steps": 25},
                    },
                ],
            )
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    saved_payload = json.loads(cube_path.read_text(encoding="utf-8"))
    response_payload = decode_json_response(response)

    assert response.status == 200
    assert saved_payload["flavors"]["authored"] == [
        {
            "id": "default",
            "name": "Default",
            "values": {"ksampler.cfg": 7},
        },
        {
            "id": "portrait",
            "name": "Portrait",
            "values": {"ksampler.cfg": 8},
        },
    ]
    assert "ksampler.steps" not in saved_payload["flavors"]["authored"][0]["values"]
    assert "ksampler.seed" not in saved_payload["flavors"]["authored"][0]["values"]
    assert saved_payload["surface"]["controls"] == exported_controls
    finalized_cube = response_payload["saved"][0]["definition"]["cube"]
    assert finalized_cube["surface"] == saved_payload["surface"]
    assert finalized_cube["flavors"] == saved_payload["flavors"]
    assert finalized_cube["implementation"] == saved_payload["implementation"]


def test_save_many_existing_implementation_save_reorders_authored_values_to_exported_surface(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    """Implementation save uses exported surface order while preserving values."""

    exported_controls = [
        _surface_control("ksampler.brightness", input_name="brightness"),
        _surface_control("ksampler.r", input_name="r"),
        _surface_control("ksampler.g", input_name="g"),
        _surface_control("ksampler.b", input_name="b"),
    ]
    bad_existing_controls = [
        _surface_control("ksampler.b", input_name="b"),
        _surface_control("ksampler.brightness", input_name="brightness"),
        _surface_control("ksampler.g", input_name="g"),
        _surface_control("ksampler.r", input_name="r"),
    ]
    bad_existing_values = {
        "ksampler.b": 4,
        "ksampler.brightness": 1,
        "ksampler.g": 3,
        "ksampler.r": 2,
    }
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            version="1.0.1",
            controls=exported_controls,
            authored=[
                {
                    "id": "default",
                    "name": "Default",
                    "values": {
                        "ksampler.brightness": 101,
                        "ksampler.r": 102,
                        "ksampler.g": 103,
                        "ksampler.b": 104,
                    },
                }
            ],
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.0.1", reason="Implementation changed", bump="minor"
        ),
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            _surface_cube_payload(
                version="1.0.0",
                controls=bad_existing_controls,
                authored=[
                    {
                        "id": "default",
                        "name": "Default",
                        "values": bad_existing_values,
                    }
                ],
            )
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    saved_payload = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert saved_payload["surface"]["controls"] == exported_controls
    assert saved_payload["flavors"]["authored"][0]["values"] == {
        "ksampler.brightness": 1,
        "ksampler.r": 2,
        "ksampler.g": 3,
        "ksampler.b": 4,
    }


def test_save_many_first_time_save_keeps_exported_default_flavor_values(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            version="1.0.0",
            authored=[
                {
                    "id": "default",
                    "name": "Default",
                    "values": {"ksampler.cfg": 11},
                }
            ],
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    cube_path = checkout / "demo.cube"

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    saved_payload = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert saved_payload["flavors"]["authored"][0]["values"] == {"ksampler.cfg": 11}


def test_save_many_first_time_save_strips_machine_local_authored_defaults(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    controls = [
        _surface_control("ksampler.cfg", input_name="cfg"),
        _surface_control("ksampler.seed", input_name="seed"),
        _surface_control(
            "checkpoint.ckpt_name",
            input_name="ckpt_name",
            class_type="CheckpointLoaderSimple",
            symbol="checkpoint",
            value_type="string",
        ),
        _surface_control(
            "vae.vae_name",
            input_name="vae_name",
            class_type="VAELoader",
            symbol="vae",
            value_type="string",
        ),
        _surface_control(
            "sam.sam_model",
            input_name="sam_model",
            class_type="SimpleSyrup.SAMModelLoader",
            symbol="sam",
            value_type="string",
        ),
    ]
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            version="1.0.0",
            controls=controls,
            authored=[
                {
                    "id": "default",
                    "name": "Default",
                    "values": {
                        "ksampler.cfg": 11,
                        "ksampler.seed": 12345,
                        "checkpoint.ckpt_name": "local.safetensors",
                        "vae.vae_name": "local-vae.safetensors",
                        "sam.sam_model": "sam_vit_b",
                    },
                }
            ],
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    cube_path = checkout / "demo.cube"

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    saved_payload = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert saved_payload["flavors"]["authored"][0]["values"] == {
        "ksampler.cfg": 11,
        "sam.sam_model": "sam_vit_b",
    }
    assert saved_payload["surface"]["controls"] == controls


def test_save_many_existing_implementation_strips_machine_local_defaults(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    controls = [
        _surface_control("ksampler.cfg", input_name="cfg"),
        _surface_control(
            "checkpoint.ckpt_name",
            input_name="ckpt_name",
            class_type="CheckpointLoaderSimple",
            symbol="checkpoint",
            value_type="string",
        ),
        _surface_control(
            "vae.vae_name",
            input_name="vae_name",
            class_type="VAELoader",
            symbol="vae",
            value_type="string",
        ),
        _surface_control(
            "sam.sam_model",
            input_name="sam_model",
            class_type="SimpleSyrup.SAMModelLoader",
            symbol="sam",
            value_type="string",
        ),
    ]
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            version="1.0.1",
            controls=controls,
            authored=[
                {
                    "id": "default",
                    "name": "Default",
                    "values": {
                        "ksampler.cfg": 99,
                        "checkpoint.ckpt_name": "exported.safetensors",
                        "vae.vae_name": "exported-vae.safetensors",
                        "sam.sam_model": "sam_vit_b",
                    },
                }
            ],
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.0.1", reason="Implementation changed", bump="minor"
        ),
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            _surface_cube_payload(
                version="1.0.0",
                controls=controls,
                authored=[
                    {
                        "id": "default",
                        "name": "Default",
                        "values": {
                            "ksampler.cfg": 7,
                            "checkpoint.ckpt_name": "existing.safetensors",
                            "vae.vae_name": "existing-vae.safetensors",
                            "sam.sam_model": "sam_vit_l",
                        },
                    },
                    {
                        "id": "portrait",
                        "name": "Portrait",
                        "values": {
                            "ksampler.cfg": 8,
                            "checkpoint.ckpt_name": "portrait.safetensors",
                            "sam.sam_model": "sam_vit_h",
                        },
                    },
                ],
            )
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    saved_payload = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert saved_payload["flavors"]["authored"] == [
        {
            "id": "default",
            "name": "Default",
            "values": {
                "ksampler.cfg": 7,
                "sam.sam_model": "sam_vit_l",
            },
        },
        {
            "id": "portrait",
            "name": "Portrait",
            "values": {
                "ksampler.cfg": 8,
                "sam.sam_model": "sam_vit_h",
            },
        },
    ]


def test_save_authored_flavor_updates_tracked_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(
        tmp_path,
        write_cube_to_path=write_cube_to_path,
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.0.1", reason="Authored flavor changed", bump="patch"
        ),
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
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
                    "layout": {
                        "groups": [
                            {
                                "sugarcubes": {
                                    "cube_id": CANONICAL_CUBE_ID,
                                    "cube_version": "1.0.0",
                                    "cube_definition_key": f"{CANONICAL_CUBE_ID}@1.0.0",
                                }
                            }
                        ]
                    },
                    "definitions": {},
                    "subgraphs": [],
                },
                "surface": {
                    "default_flavor_id": "default",
                    "controls": [
                        {
                            "control_id": "ksampler.cfg",
                            "symbol": "ksampler",
                            "input_name": "cfg",
                            "label": "cfg",
                            "class_type": "KSampler",
                            "value_type": "number",
                        }
                    ],
                },
                "flavors": {
                    "authored": [
                        {
                            "id": "default",
                            "name": "Default",
                            "values": {"ksampler.cfg": 7},
                        }
                    ]
                },
            }
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).save_authored_flavor(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "flavor_id": "default",
                    "flavor_name": "Default",
                    "values": {"ksampler.cfg": 8},
                }
            )
        )
    )
    payload = decode_json_response(response)
    saved_payload = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert payload["saved"]["cube_id"] == CANONICAL_CUBE_ID
    assert payload["saved"]["version"] == "1.0.1"
    assert saved_payload["flavors"]["authored"][0]["values"] == {"ksampler.cfg": 8}
    group_metadata = saved_payload["implementation"]["layout"]["groups"][0][
        "sugarcubes"
    ]
    assert group_metadata["cube_version"] == "1.0.1"
    assert group_metadata["cube_definition_key"] == f"{CANONICAL_CUBE_ID}@1.0.1"


def test_save_authored_flavor_strips_machine_local_defaults(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(
        tmp_path,
        write_cube_to_path=write_cube_to_path,
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.0.1", reason="Authored flavor changed", bump="patch"
        ),
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = ensure_tracked_repo(services)
    cube_path = checkout / "demo.cube"
    cube_path.write_text(
        json.dumps(
            _surface_cube_payload(
                controls=[
                    _surface_control("ksampler.cfg", input_name="cfg"),
                    _surface_control("ksampler.seed", input_name="seed"),
                    _surface_control(
                        "checkpoint.ckpt_name",
                        input_name="ckpt_name",
                        class_type="CheckpointLoaderSimple",
                        symbol="checkpoint",
                        value_type="string",
                    ),
                    _surface_control(
                        "sam.sam_model",
                        input_name="sam_model",
                        class_type="SimpleSyrup.SAMModelLoader",
                        symbol="sam",
                        value_type="string",
                    ),
                ],
                authored=[
                    {
                        "id": "default",
                        "name": "Default",
                        "values": {
                            "ksampler.cfg": 7,
                            "ksampler.seed": 12345,
                            "checkpoint.ckpt_name": "old.safetensors",
                            "sam.sam_model": "sam_vit_l",
                        },
                    }
                ],
            )
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).save_authored_flavor(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "flavor_id": "default",
                    "flavor_name": "Default",
                    "values": {
                        "ksampler.cfg": 8,
                        "ksampler.seed": 99999,
                        "checkpoint.ckpt_name": "new.safetensors",
                        "sam.sam_model": "sam_vit_b",
                    },
                }
            )
        )
    )
    saved_payload = json.loads(cube_path.read_text(encoding="utf-8"))

    assert response.status == 200
    assert saved_payload["flavors"]["authored"][0]["values"] == {
        "ksampler.cfg": 8,
        "sam.sam_model": "sam_vit_b",
    }
    assert any(
        control["control_id"] == "ksampler.seed"
        for control in saved_payload["surface"]["controls"]
    )


def test_save_authored_flavor_commits_version_changed_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(
        tmp_path,
        write_cube_to_path=write_cube_to_path,
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.0.1", reason="Authored flavor changed", bump="patch"
        ),
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _init_git_repo(services, checkout)
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
                "surface": {
                    "default_flavor_id": "default",
                    "controls": [
                        {
                            "control_id": "ksampler.cfg",
                            "symbol": "ksampler",
                            "input_name": "cfg",
                            "label": "cfg",
                            "class_type": "KSampler",
                            "value_type": "number",
                        }
                    ],
                },
                "flavors": {
                    "authored": [
                        {
                            "id": "default",
                            "name": "Default",
                            "values": {"ksampler.cfg": 7},
                        }
                    ]
                },
            }
        ),
        encoding="utf-8",
    )

    response = asyncio.run(
        build_route_handlers(services).save_authored_flavor(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "flavor_id": "default",
                    "flavor_name": "Default",
                    "values": {"ksampler.cfg": 8},
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["saved"]["committed"] is True
    assert payload["saved"]["commit_message"] == "update demo.cube v1.0.1"


def test_save_many_persists_to_managed_local_workspace(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    local_cube_id = "local/example-user/private/demo.cube"
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(cube_id=local_cube_id, version="1.0.0"),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=lambda cube_targets, overwrite=True: [
            {
                "path": str(target_path),
                "filename": target_path.name,
                "default_alias": exported.default_alias,
            }
            for _, target_path in cube_targets
        ],
        node_class_mappings={"KSampler": object()},
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": local_cube_id, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["saved"][0]["cube_id"] == local_cube_id
    saved_path = payload["saved"][0]["path"].replace("\\", "/")
    assert saved_path.endswith(".sugarcubes/local/example-user/private/demo.cube")


def test_save_many_commits_same_version_changed_local_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    local_cube_id = "local/example-user/private/demo.cube"
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(
            cube_id=local_cube_id,
            version="1.0.0",
            description="changed description",
        ),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"KSampler": object()},
    )
    cube_path = (
        services.library.local_workspace_root()
        / "example-user"
        / "private"
        / "demo.cube"
    )
    _write_current_cube(cube_path, cube_id=local_cube_id, version="1.0.0")

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": local_cube_id, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["saved"][0]["committed"] is True
    assert payload["saved"][0]["commit_sha"]
    assert payload["saved"][0]["commit_message"] == "update demo.cube content"
    assert (
        services.tracked_repos.git_runner(
            ["status", "--short"], cwd=services.library.local_workspace_root()
        ).stdout.strip()
        == ""
    )


def test_save_many_does_not_commit_noop_local_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    local_cube_id = "local/example-user/private/demo.cube"
    cube_payload = _surface_cube_payload(
        cube_id=local_cube_id,
        version="1.0.0",
        metadata={"default_alias": "private/demo"},
    )
    exported = ExportedCube(
        default_alias="Demo",
        cube=json.loads(json.dumps(cube_payload)),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"KSampler": object()},
    )
    local_root = services.library.local_workspace_root()
    cube_path = local_root / "example-user" / "private" / "demo.cube"
    write_cube_to_path(exported, cube_path, overwrite=True)
    _commit_all(services, local_root)

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": local_cube_id, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 200
    assert payload["saved"][0]["committed"] is False
    assert payload["saved"][0]["commit_sha"] == ""
    assert payload["saved"][0]["commit_error"] == ""
    assert (
        services.tracked_repos.git_runner(
            ["status", "--short"], cwd=local_root
        ).stdout.strip()
        == ""
    )


def test_save_many_commits_same_version_layout_only_github_cube(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    existing_payload = _surface_cube_payload(
        version="1.1.0",
        metadata={"default_alias": "demo"},
    )
    existing_payload["implementation"]["layout"] = {
        "origin": [0, 0],
        "ds": {"scale": 1.0, "offset": [0, 0]},
    }
    next_payload = json.loads(json.dumps(existing_payload))
    next_payload["implementation"]["layout"] = {
        "origin": [0, 0],
        "ds": {"scale": 0.5, "offset": [200, 300]},
    }
    exported = ExportedCube(
        default_alias="Demo",
        cube=next_payload,
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        suggest_version=lambda existing, current: SimpleNamespace(
            suggested="1.1.0", reason="same", bump="patch"
        ),
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _init_git_repo(services, checkout)
    baseline = ExportedCube(default_alias="Demo", cube=existing_payload, warnings=[])
    write_cube_to_path(baseline, checkout / "demo.cube", overwrite=True)
    _commit_all(services, checkout)

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)
    saved_payload = json.loads((checkout / "demo.cube").read_text(encoding="utf-8"))

    assert response.status == 200
    assert payload["saved"][0]["committed"] is True
    assert payload["saved"][0]["commit_sha"]
    assert payload["saved"][0]["commit_short_sha"]
    assert payload["saved"][0]["commit_message"] == "update demo.cube layout"
    assert saved_payload["version"] == "1.1.0"
    assert (
        services.tracked_repos.git_runner(
            ["status", "--short"], cwd=checkout
        ).stdout.strip()
        == ""
    )


def test_save_many_persists_to_new_authoring_pack_checkout(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    cube_id = "ExampleUser/Example-Cubes/text_to_image.cube"
    exported = ExportedCube(
        default_alias="Text to Image",
        cube=_surface_cube_payload(cube_id=cube_id, version="1.0.0"),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(services, owner="ExampleUser")
    services.tracked_repos.ensure_authoring_repo(
        owner="ExampleUser", repo="Example-Cubes"
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": cube_id, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)

    expected_path = (
        services.tracked_repos.checkout_path("ExampleUser", "Example-Cubes")
        / "text_to_image.cube"
    )
    assert response.status == 200
    assert payload["saved"][0]["cube_id"] == cube_id
    assert payload["saved"][0]["path"] == str(expected_path)
    assert expected_path.exists()


def test_save_many_reports_commit_failure_when_repo_has_unrelated_staged_changes(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    exported = ExportedCube(
        default_alias="Demo",
        cube=_surface_cube_payload(version="1.0.1"),
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"KSampler": object()},
    )
    claim_github_owner(
        services, owner="Artificial-Sweetener", allow_system_owner_claim=True
    )
    checkout = services.tracked_repos.checkout_path(
        "Artificial-Sweetener", "Base-Cubes"
    )
    _init_git_repo(services, checkout)
    _write_current_cube(
        checkout / "demo.cube", version="1.0.0", metadata={"author": "Alice"}
    )
    notes_path = checkout / "notes.txt"
    notes_path.write_text("pending\n", encoding="utf-8")
    services.tracked_repos.git_runner(["add", "--", "notes.txt"], cwd=checkout)

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)
    saved_entry = payload["saved"][0]
    saved_payload = json.loads((checkout / "demo.cube").read_text(encoding="utf-8"))

    assert response.status == 200
    assert saved_entry["committed"] is False
    assert "unrelated staged changes" in saved_entry["commit_error"]
    assert saved_payload["version"] == "1.0.1"


def test_save_many_rejects_noncanonical_cube_ids(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(
        tmp_path, node_class_mappings={"KSampler": object()}
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": "local/demo", "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 400
    assert (
        payload["error"]["message"]
        == "Cube id must use canonical local/<namespace>/<path>.cube format"
    )


def test_save_many_rejects_unknown_stale_save_mode(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(
        tmp_path, node_class_mappings={"KSampler": object()}
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "actor": {"author": "Alice"},
                    "cubes": [
                        {
                            "cube_id": CANONICAL_CUBE_ID,
                            "forked": False,
                            "stale_save_mode": "replace",
                        }
                    ],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 400
    assert (
        payload["error"]["message"]
        == "'stale_save_mode' must be 'latest' when provided"
    )


def test_save_many_rejects_target_model_metadata_that_conflicts_with_cube_path(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    services = backend_services_factory(
        tmp_path, node_class_mappings={"KSampler": object()}
    )

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "actor": {"author": "Alice"},
                    "cubes": [
                        {
                            "cube_id": "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube",
                            "forked": False,
                            "metadata": {"target_model": "Flux"},
                        }
                    ],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 400
    assert payload["error"]["message"] == (
        "metadata.target_model must match the cube id path"
    )


def test_load_route_uses_tracked_repo_source_metadata(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    loaded_cube = SimpleNamespace(version="1.0.0")
    prepared = SimpleNamespace(
        cube={"cube_id": CANONICAL_CUBE_ID, "version": "1.0.0"},
        nodes=[{"symbol": "node"}],
        markers=[{"alias": "input.value"}],
        connections=[{"kind": "binding"}],
        layout={"origin": [10, 20]},
        warnings=["warn"],
        subgraphs=[{"id": "subgraph"}],
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
    _write_current_cube(checkout / "demo.cube")

    handlers = build_route_handlers(services)
    success_response = asyncio.run(
        handlers.load_cube(
            FakeRequest(
                body={
                    "cube_id": CANONICAL_CUBE_ID,
                    "origin": {"x": 10, "y": 20},
                }
            )
        )
    )
    success_payload = decode_json_response(success_response)

    assert success_response.status == 200
    assert success_payload["cube"]["cube_id"] == CANONICAL_CUBE_ID
    assert success_payload["source"]["relative_path"] == "demo.cube"
    assert success_payload["source"]["repo_ref"] == "Artificial-Sweetener/Base-Cubes"


def test_import_cube_file_preserves_original_import_error_when_cleanup_fails(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    source_path = tmp_path / "source.cube"
    source_path.write_text(
        json.dumps({"cube_id": CANONICAL_CUBE_ID, "version": "1.0.0", "nodes": {}}),
        encoding="utf-8",
    )

    services = backend_services_factory(
        tmp_path,
        load_cube_artifact=lambda path: (_ for _ in ()).throw(
            CubeImportError("bad cube")
        ),
    )
    destination = (
        services.library.local_workspace_root()
        / "example-user"
        / "imports"
        / "source.cube"
    )
    original_unlink = type(destination).unlink

    def failing_unlink(path: Any, *args: Any, **kwargs: Any) -> Any:
        if path == destination:
            raise PermissionError("locked")
        return original_unlink(path, *args, **kwargs)

    monkeypatch.setattr(type(destination), "unlink", failing_unlink)

    with caplog.at_level(
        logging.WARNING, logger="sugarcubes.backend.services.cube_library_service"
    ):
        with pytest.raises(CubeImportError, match="bad cube"):
            services.library.import_cube_file(
                source_value=str(source_path),
                target_cube_id="local/example-user/imports/source.cube",
                overwrite=False,
            )

    assert destination.exists()
    assert any(
        "unable to clean up failed import" in record.message
        for record in caplog.records
    )


def test_import_cube_file_rehomes_to_canonical_local_cube_id(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    source_path = tmp_path / "source.cube"
    source_cube_id = "artificial-sweetener/base-cubes/demo.cube"
    source_path.write_text(
        json.dumps(
            {
                "cube_id": source_cube_id,
                "version": "1.0.0",
                "implementation": {
                    "nodes": {},
                    "inputs": {},
                    "outputs": {},
                    "layout": {
                        "groups": [
                            {
                                "sugarcubes": {
                                    "cube_id": source_cube_id,
                                    "cube_version": "0.9.0",
                                    "cube_definition_key": f"{source_cube_id}@0.9.0",
                                }
                            }
                        ]
                    },
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
    services = backend_services_factory(tmp_path)

    response = asyncio.run(
        build_route_handlers(services).import_cube_file(
            FakeRequest(
                body={
                    "path": str(source_path),
                    "cube_id": "local/example-user/imports/source.cube",
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 201
    assert payload["cube"]["cube_id"] == "local/example-user/imports/source.cube"
    imported_path = (
        services.library.local_workspace_root()
        / "example-user"
        / "imports"
        / "source.cube"
    )
    imported_payload = json.loads(imported_path.read_text(encoding="utf-8"))
    assert imported_payload["cube_id"] == "local/example-user/imports/source.cube"
    imported_group_metadata = imported_payload["implementation"]["layout"]["groups"][0][
        "sugarcubes"
    ]
    assert (
        imported_group_metadata["cube_id"] == "local/example-user/imports/source.cube"
    )
    assert imported_group_metadata["cube_version"] == "1.0.0"
    assert imported_group_metadata["cube_definition_key"] == (
        "local/example-user/imports/source.cube@1.0.0"
    )


def test_save_many_rejects_read_only_tracked_repo_targets(
    tmp_path: Path, backend_services_factory: BackendServicesFactory
) -> None:
    exported = ExportedCube(
        default_alias="Demo",
        cube={"cube_id": CANONICAL_CUBE_ID, "version": "1.0.0", "metadata": {}},
        warnings=[],
        version_auto=False,
    )
    services = backend_services_factory(
        tmp_path,
        export_cubes=lambda *args, **kwargs: [exported],
        write_cubes_to_paths=write_cubes_to_paths,
        node_class_mappings={"KSampler": object()},
    )
    ensure_tracked_repo(services)

    response = asyncio.run(
        build_route_handlers(services).save_many(
            FakeRequest(
                body={
                    "graph": {"1": {"class_type": "KSampler", "inputs": {}}},
                    "workflow": {"definitions": {"subgraphs": []}},
                    "workflow_version": 1,
                    "actor": {"author": "Alice"},
                    "cubes": [{"cube_id": CANONICAL_CUBE_ID, "forked": False}],
                }
            )
        )
    )
    payload = decode_json_response(response)

    assert response.status == 403
    assert "read-only" in payload["error"]["message"]
