#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify authoring plans project into the canonical execution boundary."""

from __future__ import annotations

from typing import cast

import pytest
from sugarcubes.package_identity import runtime_version

from sugarcubes.authoring import (
    NativeCubeImport,
    NativeWorkflowImportPlan,
    NativeWorkflowProjectionError,
    project_native_workflow_plan,
)
from sugarcubes.execution import CubeExecutionCoordinator, CubeExecutionRequest
from sugarcubes.language.compiler_models import (
    CompiledCubeConnection,
    CompiledFieldAnnotation,
)


def test_projected_plan_lowers_named_values_and_public_links() -> None:
    """Preserve authoring semantics through the same execution coordinator as Comfy."""

    plan = NativeWorkflowImportPlan(
        "a" * 64,
        (
            NativeCubeImport("source", "Source", False, _payload("Source", 17)),
            NativeCubeImport("target", "Target", False, _payload("Target", 23)),
        ),
        (CompiledCubeConnection("source", "output.image", "target", "input.image"),),
    )

    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=project_native_workflow_plan(plan))
    )

    assert prepared.prompt["source:image"]["inputs"] == {
        "image": None,
        "strength": 17,
    }
    assert prepared.prompt["target:image"]["inputs"] == {
        "image": ["source:image", 0],
        "strength": 23,
    }


def test_projection_marks_every_top_level_cube_with_sugarcubes_node_pack() -> None:
    """Persist the extension identity separately from each Cube artifact version."""

    plan = NativeWorkflowImportPlan(
        "a" * 64,
        (
            NativeCubeImport("first", "First", False, _payload("First", 1)),
            NativeCubeImport("second", "Second", False, _payload("Second", 2)),
        ),
        (),
    )

    workflow = project_native_workflow_plan(plan)
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    properties = [cast(dict[str, object], node["properties"]) for node in nodes]

    assert [(item["cnr_id"], item["ver"]) for item in properties] == [
        ("SugarCubes", runtime_version())
    ] * 2
    assert all(
        cast(dict[str, object], item["sugarcubes_cube"])["cube_version"] == "1.0.0"
        for item in properties
    )


def test_projection_persists_opaque_field_annotations_without_graph_edges() -> None:
    """Keep Substitute model identity beside stable fields as graph metadata only."""

    annotation = CompiledFieldAnnotation(
        instance_id="first",
        node_symbol="image",
        input_name="strength",
        namespace="substitute.model_asset",
        payload={"sha256": "A" * 64},
    )
    plan = NativeWorkflowImportPlan(
        "a" * 64,
        (NativeCubeImport("first", "First", False, _payload("First", 1)),),
        (),
        (annotation,),
    )

    workflow = project_native_workflow_plan(plan)

    assert workflow["links"] == []
    assert cast(dict[str, object], workflow["extra"])["sugarcubes_composition"] == {
        "schema_version": 1,
        "field_annotations": [
            {
                "annotation_id": "substitute.model_asset:first:image:strength",
                "namespace": "substitute.model_asset",
                "endpoint": {
                    "instance_id": "first",
                    "node_symbol": "image",
                    "input_name": "strength",
                },
                "payload": {"sha256": "A" * 64},
            }
        ],
    }


def test_projection_reconstructs_implicit_series_as_typed_proximity() -> None:
    """Let SugarCubes recover stack adjacency without persisted explicit links."""

    plan = NativeWorkflowImportPlan(
        "a" * 64,
        (
            NativeCubeImport("source", "Source", False, _payload("Source", 17)),
            NativeCubeImport("target", "Target", False, _payload("Target", 23)),
        ),
        (),
    )

    workflow = project_native_workflow_plan(plan)
    prepared = CubeExecutionCoordinator().prepare(
        CubeExecutionRequest(workflow=workflow)
    )

    nodes = cast(list[dict[str, object]], workflow["nodes"])
    assert nodes[0]["outputs"] == [{"name": "output.image", "type": "IMAGE"}]
    assert nodes[1]["inputs"] == [{"name": "input.image", "type": "IMAGE"}]
    assert nodes[0]["pos"] == [0.0, 0.0]
    assert nodes[1]["pos"] == [344.0, 0.0]
    assert workflow["links"] == []
    target = prepared.prompt["target:image"]
    inputs = cast(dict[str, object], target["inputs"])
    assert inputs["image"] == [
        "source:image",
        0,
    ]


def test_projection_rejects_unknown_boundary_without_slot_guessing() -> None:
    """Fail before execution when a connection name does not exist exactly."""

    plan = NativeWorkflowImportPlan(
        "a" * 64,
        (
            NativeCubeImport("source", "Source", False, _payload("Source", 1)),
            NativeCubeImport("target", "Target", False, _payload("Target", 2)),
        ),
        (CompiledCubeConnection("source", "image", "target", "input.image"),),
    )

    with pytest.raises(NativeWorkflowProjectionError, match="output boundary 'image'"):
        project_native_workflow_plan(plan)


def _payload(alias: str, strength: int) -> dict[str, object]:
    """Build one prepared-plan payload with a complete portable document."""

    document: dict[str, object] = {
        "cube_id": f"owner/repo/{alias}.cube",
        "version": "1.0.0",
        "metadata": {"default_alias": alias},
        "implementation": {
            "nodes": {
                "image": {
                    "class_type": "ImagePass",
                    "inputs": {"image": None, "strength": strength},
                }
            },
            "inputs": {
                "input.image": {
                    "kind": "input",
                    "targets": [["image", "image"]],
                }
            },
            "outputs": {"output.image": ["image", 0]},
            "layout": {},
            "definitions": {
                "ImagePass": {
                    "input": {
                        "required": {
                            "image": ["IMAGE"],
                            "strength": ["INT", {"default": 1}],
                        }
                    },
                    "output": ["IMAGE"],
                }
            },
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }
    return {"document": document}
