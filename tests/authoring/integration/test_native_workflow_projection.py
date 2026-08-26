#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify authoring plans project into the canonical execution boundary."""

from __future__ import annotations

import pytest
from sugarcubes.package_identity import runtime_version

from sugarcubes.authoring import (
    NativeCubeImport,
    NativeWorkflowImportPlan,
    NativeWorkflowProjectionError,
    project_native_workflow_plan,
)
from sugarcubes.execution import CubeExecutionCoordinator, CubeExecutionRequest
from sugarcubes.language.compiler_models import CompiledCubeConnection


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
    nodes = workflow["nodes"]
    assert isinstance(nodes, list)

    assert [
        (node["properties"]["cnr_id"], node["properties"]["ver"]) for node in nodes
    ] == [("SugarCubes", runtime_version())] * 2
    assert all(
        node["properties"]["sugarcubes_cube"]["cube_version"] == "1.0.0"
        for node in nodes
    )


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
