#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify the versioned canonical workflow analysis HTTP adapter."""

from __future__ import annotations

import asyncio
from pathlib import Path
from typing import cast

from sugarcubes.backend.routes import build_route_handlers
from tests.backend_api.support.backend_fixtures import FakeRequest, decode_json_response
from tests.backend_api.support.typing_support import BackendServicesFactory
from tests.execution.support.execution_fixtures import (
    cube_workflow,
    image_passthrough_document,
)


def test_analysis_route_returns_one_complete_cube_projection(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Expose headless proximity and reorder segments in one response."""

    workflow = cube_workflow(
        {
            "source": image_passthrough_document("Source"),
            "target": image_passthrough_document("Target"),
        }
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    nodes[0].update({"pos": [0, 0], "size": [100, 100]})
    nodes[1].update({"pos": [220, 0], "size": [100, 100]})
    nodes[0]["outputs"] = [{"name": "output.image", "type": "IMAGE", "links": None}]
    nodes[1]["inputs"] = [{"name": "input.image", "type": "IMAGE", "link": None}]

    response = asyncio.run(
        build_route_handlers(backend_services_factory(tmp_path)).analyze_workflow(
            FakeRequest(body={"schema_version": 1, "workflow": workflow})
        )
    )

    assert response.status == 200
    payload = decode_json_response(response)
    assert payload["schema_version"] == 1
    assert payload["workflow"]["extra"] == {"fixture": True}
    assert [instance["instance_id"] for instance in payload["instances"]] == [
        "source",
        "target",
    ]
    assert payload["edges"] == [
        {
            "source_instance_id": "source",
            "source_binding": "output.image",
            "target_instance_id": "target",
            "target_binding": "input.image",
            "origin": "proximity",
        }
    ]
    assert payload["segments"] == [
        {
            "instance_ids": ["source", "target"],
            "reorderable": True,
            "boundary_node_ids": [],
        }
    ]


def test_analysis_route_rejects_incomplete_workflows_with_location(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Fail closed before a client can project incomplete graph state."""

    response = asyncio.run(
        build_route_handlers(backend_services_factory(tmp_path)).analyze_workflow(
            FakeRequest(body={"schema_version": 1, "workflow": {}})
        )
    )

    assert response.status == 422
    assert decode_json_response(response)["error"] == {
        "message": "Workflow value at '$.nodes' must be an array.",
        "code": "workflow.invalid_nodes",
        "phase": "workflow",
        "path": "$.nodes",
    }


def test_reorder_route_returns_mutated_graph_and_refreshed_analysis(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Expose one atomic proximity-only stack mutation at the HTTP boundary."""

    workflow = cube_workflow(
        {
            "source": image_passthrough_document("Source"),
            "target": image_passthrough_document("Target"),
        }
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for index, node in enumerate(nodes):
        node.update({"pos": [index * 124, 0], "size": [100, 100]})
        node["inputs"] = [{"name": "input.image", "type": "IMAGE", "link": None}]
        node["outputs"] = [{"name": "output.image", "type": "IMAGE", "links": None}]

    response = asyncio.run(
        build_route_handlers(backend_services_factory(tmp_path)).reorder_workflow(
            FakeRequest(
                body={
                    "schema_version": 1,
                    "workflow": workflow,
                    "segment_instance_ids": ["source", "target"],
                    "ordered_instance_ids": ["target", "source"],
                }
            )
        )
    )

    assert response.status == 200
    payload = decode_json_response(response)
    segments = cast(list[dict[str, object]], payload["segments"])
    normalized_workflow = cast(dict[str, object], payload["workflow"])
    assert segments[0]["instance_ids"] == ["target", "source"]
    assert normalized_workflow["extra"] == {"fixture": True}


def test_append_and_remove_routes_keep_non_cube_graph_content_opaque(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Expose exact Cube mutations without rewriting ordinary Comfy records."""

    workflow = {
        "version": 0.4,
        "nodes": [
            {
                "id": "ordinary",
                "type": "VendorNode",
                "properties": {"vendor": {"opaque": True}},
            }
        ],
        "links": [],
        "definitions": {"subgraphs": []},
        "extra": {"vendor": [1, 2, 3]},
    }
    handlers = build_route_handlers(backend_services_factory(tmp_path))

    appended_response = asyncio.run(
        handlers.append_workflow_cube(
            FakeRequest(
                body={
                    "schema_version": 1,
                    "workflow": workflow,
                    "cube": {
                        "instance_id": "added",
                        "alias": "Added",
                        "bypassed": False,
                        "document": image_passthrough_document("Added"),
                    },
                }
            )
        )
    )

    assert appended_response.status == 200
    appended = decode_json_response(appended_response)
    appended_workflow = cast(dict[str, object], appended["workflow"])
    appended_nodes = cast(list[dict[str, object]], appended_workflow["nodes"])
    original_nodes = cast(list[dict[str, object]], workflow["nodes"])
    appended_instances = cast(list[dict[str, object]], appended["instances"])
    assert appended_nodes[0] == original_nodes[0]
    assert appended_workflow["extra"] == workflow["extra"]
    assert appended_instances[0]["instance_id"] == "added"

    removed_response = asyncio.run(
        handlers.remove_workflow_cube(
            FakeRequest(
                body={
                    "schema_version": 1,
                    "workflow": appended_workflow,
                    "instance_id": "added",
                }
            )
        )
    )

    assert removed_response.status == 200
    removed = decode_json_response(removed_response)
    assert removed["workflow"] == workflow
    assert removed["instances"] == []


def test_replace_route_preserves_instance_identity_and_updates_version(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Expose canonical replacement without detaching the native owner node."""

    workflow = cube_workflow({"cube": image_passthrough_document("Cube")})
    replacement = image_passthrough_document("Cube")
    replacement["version"] = "1.1.0"
    handlers = build_route_handlers(backend_services_factory(tmp_path))

    response = asyncio.run(
        handlers.replace_workflow_cube(
            FakeRequest(
                body={
                    "schema_version": 1,
                    "workflow": workflow,
                    "instance_id": "cube",
                    "document": replacement,
                }
            )
        )
    )

    assert response.status == 200
    payload = decode_json_response(response)
    instances = cast(list[dict[str, object]], payload["instances"])
    assert instances[0]["instance_id"] == "cube"
    assert instances[0]["node_id"] == "1"
    assert instances[0]["cube_version"] == "1.1.0"


def test_create_cube_workflow_route_returns_one_canonical_graph(
    tmp_path: Path,
    backend_services_factory: BackendServicesFactory,
) -> None:
    """Expose one bounded legacy-stack migration request."""

    response = asyncio.run(
        build_route_handlers(backend_services_factory(tmp_path)).create_cube_workflow(
            FakeRequest(
                body={
                    "schema_version": 1,
                    "cubes": [
                        {
                            "instance_id": "first",
                            "alias": "First",
                            "bypassed": False,
                            "document": image_passthrough_document("First"),
                        },
                        {
                            "instance_id": "second",
                            "alias": "Second",
                            "bypassed": False,
                            "document": image_passthrough_document("Second"),
                        },
                    ],
                }
            )
        )
    )

    assert response.status == 200
    payload = decode_json_response(response)
    assert [item["instance_id"] for item in payload["instances"]] == [
        "first",
        "second",
    ]
    assert payload["segments"][0]["instance_ids"] == ["first", "second"]
