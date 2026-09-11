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
"""Verify the bounded SugarScript compile route and native import response."""

from __future__ import annotations

import asyncio
import json
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from sugarcubes.backend.routes import build_route_handlers
from sugarcubes.importer import load_cube, prepare_import
from tests.backend_api.support.backend_fixtures import (
    FakeRequest,
    decode_json_response,
    ensure_tracked_repo,
)
from tests.backend_api.support.typing_support import BackendServicesFactory

CUBE_ID = "Artificial-Sweetener/Base-Cubes/contract.cube"


def test_compile_route_resolves_catalog_cube_and_returns_native_import_plan(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Expose prepared native Cube construction without changing a workflow."""

    services = backend_services_factory(
        tmp_path,
        load_cube_artifact=load_cube,
        prepare_cube_import=prepare_import,
    )
    checkout = ensure_tracked_repo(services)
    _write_cube(checkout / "contract.cube")
    response = asyncio.run(
        build_route_handlers(services).compile_sugarscript(
            FakeRequest(
                body={
                    "source": f'use "{CUBE_ID}" as Native\nset Native.sampler.steps = 24\n'
                }
            )
        )
    )

    assert response.status == 200
    body = _mapping(decode_json_response(response))
    assert body["diagnostics"] == []
    plan = _mapping(body["plan"])
    instances = plan["instances"]
    assert isinstance(instances, list)
    instance = _mapping(instances[0])
    assert instance["alias"] == "Native"
    payload = _mapping(instance["payload"])
    document = _mapping(payload["document"])
    implementation = _mapping(document["implementation"])
    nodes = _mapping(implementation["nodes"])
    sampler = _mapping(nodes["sampler"])
    assert _mapping(sampler["inputs"])["steps"] == 24
    workflow = _mapping(body["workflow"])
    workflow_nodes = workflow["nodes"]
    assert isinstance(workflow_nodes, list)
    assert _mapping(workflow_nodes[0])["type"] == "sugarcubes-plan-definition-1"
    workflow_definitions = _mapping(workflow["definitions"])["subgraphs"]
    assert isinstance(workflow_definitions, list)
    workflow_document = _mapping(
        _mapping(_mapping(workflow_definitions[0])["extra"])["sugarcubes_document"]
    )
    workflow_implementation = _mapping(workflow_document["implementation"])
    workflow_sampler = _mapping(_mapping(workflow_implementation["nodes"])["sampler"])
    assert _mapping(workflow_sampler["inputs"])["steps"] == 24
    authored_flavors = _mapping(workflow_document["flavors"])["authored"]
    assert isinstance(authored_flavors, list)
    assert _mapping(_mapping(authored_flavors[0])["values"])["sampler.steps"] == 24
    assert (
        _mapping(workflow["extra"])["sugarcubes_authoring_semantic_hash"]
        == plan["semantic_hash"]
    )
    analysis = _mapping(body["analysis"])
    analyzed_instances = analysis["instances"]
    assert isinstance(analyzed_instances, list)
    assert _mapping(analyzed_instances[0])["instance_alias"] == "Native"
    assert analysis["workflow"] == workflow


def test_compile_route_returns_located_diagnostics_and_no_plan(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Keep invalid source out of frontend graph mutation."""

    services = backend_services_factory(tmp_path)
    response = asyncio.run(
        build_route_handlers(services).compile_sugarscript(
            FakeRequest(body={"source": "set Missing.sampler.steps = 24\n"})
        )
    )

    assert response.status == 422
    body = _mapping(decode_json_response(response))
    assert body["plan"] is None
    assert body["workflow"] is None
    assert body["analysis"] is None
    diagnostics = body["diagnostics"]
    assert isinstance(diagnostics, list)
    first = _mapping(diagnostics[0])
    assert first["code"] == "sugarscript.compile.invalid_semantics"
    assert _mapping(_mapping(first["span"])["start"])["line"] == 1


def test_compile_route_rejects_non_string_and_oversized_source(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Bound untrusted authoring input before parsing or catalog access."""

    handler = build_route_handlers(
        backend_services_factory(tmp_path)
    ).compile_sugarscript
    wrong_type = asyncio.run(handler(FakeRequest(body={"source": 42})))
    oversized = asyncio.run(handler(FakeRequest(body={"source": "x" * 1_048_577})))

    assert wrong_type.status == 400
    assert oversized.status == 413


def _write_cube(path: Path) -> None:
    """Write one canonical managed Cube for route resolution."""

    payload: dict[str, Any] = {
        "cube_id": CUBE_ID,
        "version": "1.0.0",
        "metadata": {"default_alias": "contract"},
        "implementation": {
            "nodes": {
                "sampler": {
                    "class_type": "KSampler",
                    "inputs": {"steps": 10},
                }
            },
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
                    "control_id": "sampler.steps",
                    "symbol": "sampler",
                    "input_name": "steps",
                    "label": "Steps",
                    "class_type": "KSampler",
                    "value_type": "number",
                }
            ],
        },
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }
    path.write_text(json.dumps(payload), encoding="utf-8")


def _mapping(value: object) -> Mapping[str, object]:
    """Narrow one decoded JSON object for strict route assertions."""

    assert isinstance(value, Mapping)
    assert all(isinstance(key, str) for key in value)
    return value
