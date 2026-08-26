#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Specify native Cube lowering and mandatory execution-node ownership."""

from __future__ import annotations

from typing import Mapping

import pytest

from sugarcubes.execution import CubeLoweringError
from sugarcubes.execution import NativeCubeWorkflowLowerer, build_cube_topology
from sugarcubes.execution.limits import MAX_NATIVE_SCOPE_NODES
from sugarcubes.execution.native_subgraph_lowering import NativeSubgraphDocumentLowerer
from sugarcubes.workflow import read_canonical_workflow
from tests.execution.support.execution_fixtures import (
    cube_document,
    cube_workflow,
    image_passthrough_document,
)


def test_lowering_connects_public_boundaries_and_marks_loose_nodes_explicitly() -> None:
    """Lower Cube internals without inferring ownership from execution IDs."""

    workflow_value = cube_workflow(
        {
            "cube-a": image_passthrough_document("A"),
            "cube-b": image_passthrough_document("B"),
        },
        links=[[1, 1, 0, 2, 0, "IMAGE"]],
        loose_nodes=[
            {
                "id": 80,
                "type": "PreviewImage",
                "inputs": [{"name": "images", "link": None}],
                "outputs": [],
                "properties": {},
            }
        ],
    )
    workflow = read_canonical_workflow(workflow_value)
    topology = build_cube_topology(workflow)

    result = NativeCubeWorkflowLowerer().lower(workflow, topology)

    assert _inputs(result.prompt["cube-b:image"])["image"] == ["cube-a:image", 0]
    assert result.node_owners["cube-a:image"].instance_id == "cube-a"
    assert result.node_owners["cube-b:image"].instance_id == "cube-b"
    assert result.node_owners["80"].is_loose
    assert [binding.binding for binding in result.boundary_bindings] == [
        "input.image",
        "output.image",
        "input.image",
        "output.image",
    ]


def test_loose_node_widget_companion_values_fail_closed_without_named_snapshot() -> (
    None
):
    """Never shift loose-node values past an unidentified Comfy companion control."""

    workflow_value = cube_workflow(
        {},
        loose_nodes=[
            {
                "id": 80,
                "type": "KSampler",
                "inputs": [
                    {"name": "seed", "widget": {"name": "seed"}},
                    {"name": "steps", "widget": {"name": "steps"}},
                    {"name": "cfg", "widget": {"name": "cfg"}},
                ],
                "widgets_values": [123, "randomize", 30, 5.5],
            }
        ],
    )
    workflow = read_canonical_workflow(workflow_value)

    with pytest.raises(CubeLoweringError) as captured:
        NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert captured.value.code == "execution.lowering.ambiguous_loose_widget_values"
    assert "Loose workflow node '80' (KSampler)" in str(captured.value)


def test_loose_node_named_widget_snapshot_ignores_positional_companions() -> None:
    """Lower current requests from their explicit name-addressed widget snapshot."""

    workflow_value = cube_workflow(
        {},
        loose_nodes=[
            {
                "id": 80,
                "type": "KSampler",
                "inputs": [
                    {"name": "seed", "widget": {"name": "seed"}},
                    {"name": "steps", "widget": {"name": "steps"}},
                    {"name": "cfg", "widget": {"name": "cfg"}},
                ],
                "widgets_values": [123, "randomize", 30, 5.5],
                "sugarcubes_widget_values": {
                    "seed": 123,
                    "steps": 30,
                    "cfg": 5.5,
                },
            }
        ],
    )
    workflow = read_canonical_workflow(workflow_value)

    result = NativeCubeWorkflowLowerer().lower(
        workflow,
        build_cube_topology(workflow),
    )

    assert _inputs(result.prompt["80"]) == {
        "seed": 123,
        "steps": 30,
        "cfg": 5.5,
    }


def test_mixed_cube_and_loose_links_preserve_dataflow_without_cube_topology() -> None:
    """Rebind ordinary crossing links while keeping the loose-node barrier."""

    workflow_value = cube_workflow(
        {
            "cube-a": image_passthrough_document("A"),
            "cube-b": image_passthrough_document("B"),
        },
        links=[
            [1, 1, 0, 80, 0, "IMAGE"],
            [2, 80, 0, 2, 0, "IMAGE"],
        ],
        loose_nodes=[
            {
                "id": 80,
                "type": "ImagePass",
                "inputs": [{"name": "image", "link": 1}],
                "outputs": [{"name": "image", "type": "IMAGE", "links": [2]}],
                "properties": {},
            }
        ],
    )
    root_nodes = workflow_value["nodes"]
    assert isinstance(root_nodes, list)
    assert isinstance(root_nodes[1], dict)
    target_inputs = root_nodes[1]["inputs"]
    assert isinstance(target_inputs, list)
    assert isinstance(target_inputs[0], dict)
    target_inputs[0]["link"] = 2
    workflow = read_canonical_workflow(workflow_value)
    topology = build_cube_topology(workflow)

    result = NativeCubeWorkflowLowerer().lower(workflow, topology)

    assert topology.edges == ()
    assert _inputs(result.prompt["80"])["image"] == ["cube-a:image", 0]
    assert _inputs(result.prompt["cube-b:image"])["image"] == ["80", 0]


def test_loose_source_reaches_current_native_cube_boundary() -> None:
    """Keep a serialized loose link live when the target uses native state."""

    target = image_passthrough_document("Native target")
    workflow_value = cube_workflow(
        {"cube-b": target},
        links=[[1, 80, 0, 1, 0, "IMAGE"]],
        loose_nodes=[
            {
                "id": 80,
                "type": "ImagePass",
                "inputs": [{"name": "image", "link": None}],
                "outputs": [{"name": "image", "type": "IMAGE", "links": [1]}],
                "properties": {},
            }
        ],
    )
    root_nodes = workflow_value["nodes"]
    assert isinstance(root_nodes, list)
    assert isinstance(root_nodes[0], dict)
    root_nodes[0]["inputs"] = [{"name": "input.image", "type": "IMAGE", "link": 1}]
    definitions = workflow_value["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    native = subgraphs[0]
    assert isinstance(native, dict)
    native.update(
        {
            "inputs": [{"name": "input.image", "type": "IMAGE"}],
            "outputs": [{"name": "output.image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": 20,
                    "type": "ImagePass",
                    "inputs": [{"name": "image", "link": 200}],
                    "outputs": [{"name": "image", "links": [201]}],
                    "properties": {"sugarcubes_symbol": "image"},
                }
            ],
            "links": [
                {
                    "id": 200,
                    "origin_id": -10,
                    "origin_slot": 0,
                    "target_id": 20,
                    "target_slot": 0,
                },
                {
                    "id": 201,
                    "origin_id": 20,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                },
            ],
        }
    )
    workflow = read_canonical_workflow(workflow_value)

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert _inputs(result.prompt["cube-b:image"])["image"] == ["80", 0]


def test_bypass_rewires_compatible_resources_and_disabled_nodes_disappear() -> None:
    """Resolve execution modes before inheritance without changing the document."""

    document = image_passthrough_document("Bypass")
    implementation = document["implementation"]
    assert isinstance(implementation, dict)
    implementation["nodes"] = {
        "source": {"class_type": "CheckpointLoader", "inputs": {}},
        "patch": {
            "class_type": "ModelTransform",
            "inputs": {"model": ["source", 0]},
            "mode": 4,
        },
        "consumer": {
            "class_type": "ModelConsumer",
            "inputs": {"model": ["patch", 0]},
        },
        "never": {"class_type": "VaeConsumer", "inputs": {"vae": None}, "mode": 2},
    }
    implementation["inputs"] = {}
    implementation["outputs"] = {}
    workflow = read_canonical_workflow(cube_workflow({"cube-a": document}))

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert "cube-a:patch" not in result.prompt
    assert "cube-a:never" not in result.prompt
    assert _inputs(result.prompt["cube-a:consumer"])["model"] == ["cube-a:source", 0]


def test_outer_cube_bypass_is_a_typed_transparent_connection() -> None:
    """Match native Comfy bypass while keeping mode semantics in SugarCubes."""

    workflow = read_canonical_workflow(
        cube_workflow(
            {
                "cube-a": image_passthrough_document("A"),
                "cube-b": image_passthrough_document("B"),
                "cube-c": image_passthrough_document("C"),
            },
            links=[
                [1, 1, 0, 2, 0, "IMAGE"],
                [2, 2, 0, 3, 0, "IMAGE"],
            ],
            modes={"cube-b": 4},
        )
    )

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert not any(node_id.startswith("cube-b:") for node_id in result.prompt)
    assert _inputs(result.prompt["cube-c:image"])["image"] == ["cube-a:image", 0]


def test_outer_cube_disable_is_an_execution_barrier() -> None:
    """Omit disabled Cube internals without treating disable as bypass."""

    workflow = read_canonical_workflow(
        cube_workflow(
            {
                "cube-a": image_passthrough_document("A"),
                "cube-b": image_passthrough_document("B"),
                "cube-c": image_passthrough_document("C"),
            },
            links=[
                [1, 1, 0, 2, 0, "IMAGE"],
                [2, 2, 0, 3, 0, "IMAGE"],
            ],
            modes={"cube-b": 2},
        )
    )

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert not any(node_id.startswith("cube-b:") for node_id in result.prompt)
    assert _inputs(result.prompt["cube-c:image"])["image"] is None


def test_outer_cube_bypass_rejects_ambiguous_compatible_sources() -> None:
    """Improve implicit host traversal by failing closed on ambiguous bypass."""

    middle = image_passthrough_document("B")
    implementation = middle["implementation"]
    assert isinstance(implementation, dict)
    implementation["inputs"] = {
        "input.first": {"kind": "input", "targets": [["image", "image"]]},
        "input.second": {"kind": "input", "targets": [["image", "image"]]},
    }
    workflow = read_canonical_workflow(
        cube_workflow(
            {
                "cube-a": image_passthrough_document("A"),
                "cube-b": middle,
                "cube-c": image_passthrough_document("C"),
                "cube-d": image_passthrough_document("D"),
            },
            links=[
                [1, 1, 0, 2, 0, "IMAGE"],
                [2, 4, 0, 2, 1, "IMAGE"],
                [3, 2, 0, 3, 0, "IMAGE"],
            ],
            modes={"cube-b": 4},
        )
    )

    with pytest.raises(CubeLoweringError) as captured:
        NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert captured.value.code == "execution.lowering.ambiguous_outer_bypass"


def test_recursively_flattens_native_subgraphs_under_the_outer_cube_owner() -> None:
    """Remove wrapper UUIDs and retain every nested child under one Cube owner."""

    inner = {
        "id": "inner-subgraph",
        "inputs": [{"name": "image", "label": "image", "type": "IMAGE"}],
        "outputs": [{"name": "image", "label": "image", "type": "IMAGE"}],
        "nodes": [
            {
                "id": 1,
                "type": "ImagePass",
                "inputs": [{"name": "image", "link": 1}],
                "outputs": [{"name": "image", "links": [2]}],
                "properties": {"sugarcubes_symbol": "inner"},
            }
        ],
        "links": [
            {
                "id": 1,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 1,
                "target_slot": 0,
            },
            {
                "id": 2,
                "origin_id": 1,
                "origin_slot": 0,
                "target_id": -20,
                "target_slot": 0,
            },
        ],
    }
    outer = {
        "id": "outer-subgraph",
        "inputs": [{"name": "image", "label": "image", "type": "IMAGE"}],
        "outputs": [{"name": "image", "label": "image", "type": "IMAGE"}],
        "nodes": [
            {
                "id": 10,
                "type": "inner-subgraph",
                "inputs": [{"name": "image", "link": 10}],
                "outputs": [{"name": "image", "links": [11]}],
            }
        ],
        "links": [
            {
                "id": 10,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 10,
                "target_slot": 0,
            },
            {
                "id": 11,
                "origin_id": 10,
                "origin_slot": 0,
                "target_id": -20,
                "target_slot": 0,
            },
        ],
    }
    document = cube_document(
        "Nested",
        nodes={
            "wrapper": {
                "class_type": "outer-subgraph",
                "inputs": {"image": ["@binding", "input.image"]},
            },
        },
        inputs={"input.image": {"kind": "input", "targets": [["wrapper", "image"]]}},
        outputs={"output.image": ["wrapper", 0]},
        definitions={
            "ImagePass": {
                "input": {"required": {"image": ["IMAGE"]}},
                "output": ["IMAGE"],
            },
        },
        subgraphs=(inner, outer),
    )
    workflow = read_canonical_workflow(
        cube_workflow(
            {
                "cube-a": image_passthrough_document("A"),
                "cube-b": document,
                "cube-c": image_passthrough_document("C"),
            },
            links=[
                [1, 1, 0, 2, 0, "IMAGE"],
                [2, 2, 0, 3, 0, "IMAGE"],
            ],
        )
    )

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    nested_id = "cube-b:wrapper:10:inner"
    assert "cube-b:wrapper" not in result.prompt
    assert _inputs(result.prompt[nested_id])["image"] == ["cube-a:image", 0]
    assert _inputs(result.prompt["cube-c:image"])["image"] == [nested_id, 0]
    assert result.node_owners[nested_id].instance_id == "cube-b"


def test_native_bypass_uses_serialized_slot_types_without_live_definitions() -> None:
    """Retain a bypassed model link when the workflow carries only native slots."""

    sampler = {
        "id": "sampler-subgraph",
        "inputs": [{"name": "model", "type": "MODEL"}],
        "outputs": [{"name": "image", "type": "IMAGE"}],
        "nodes": [
            {
                "id": 30,
                "type": "ModelToImage",
                "inputs": [{"name": "model", "type": "MODEL", "link": 3}],
                "outputs": [{"name": "image", "type": "IMAGE", "links": [4]}],
                "properties": {"sugarcubes_symbol": "consumer"},
            }
        ],
        "links": [
            {
                "id": 3,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 30,
                "target_slot": 0,
            },
            {
                "id": 4,
                "origin_id": 30,
                "origin_slot": 0,
                "target_id": -20,
                "target_slot": 0,
            },
        ],
    }
    cube = {
        "id": "cube-definition",
        "inputs": [],
        "outputs": [{"name": "image", "type": "IMAGE"}],
        "nodes": [
            {
                "id": 10,
                "type": "ModelSource",
                "inputs": [],
                "outputs": [{"name": "MODEL", "type": "MODEL", "links": [1]}],
                "properties": {"sugarcubes_symbol": "source"},
            },
            {
                "id": 20,
                "type": "ModelVisualizer",
                "mode": 4,
                "inputs": [{"name": "model", "type": "MODEL", "link": 1}],
                "outputs": [{"name": "MODEL", "type": "MODEL", "links": [2]}],
                "properties": {"sugarcubes_symbol": "bypassed"},
            },
            {
                "id": 30,
                "type": "sampler-subgraph",
                "inputs": [{"name": "model", "type": "MODEL", "link": 2}],
                "outputs": [{"name": "image", "type": "IMAGE", "links": [5]}],
                "properties": {"sugarcubes_symbol": "wrapper"},
            },
        ],
        "links": [
            {
                "id": 1,
                "origin_id": 10,
                "origin_slot": 0,
                "target_id": 20,
                "target_slot": 0,
            },
            {
                "id": 2,
                "origin_id": 20,
                "origin_slot": 0,
                "target_id": 30,
                "target_slot": 0,
            },
            {
                "id": 5,
                "origin_id": 30,
                "origin_slot": 0,
                "target_id": -20,
                "target_slot": 0,
            },
        ],
    }

    prompt, _, _ = NativeSubgraphDocumentLowerer().lower_definition(
        "cube-a",
        None,
        cube,
        {},
        (sampler,),
    )

    assert _inputs(prompt["cube-a:wrapper:consumer"])["model"] == ["cube-a:source", 0]


def test_native_boundary_widgets_supply_hidden_wrapper_defaults() -> None:
    """Preserve embedded native defaults when the outer Cube hides controls."""

    sampler_subgraph = {
        "id": "sampler-subgraph",
        "inputs": [
            {"name": "seed", "label": "seed", "type": "INT"},
            {"name": "steps", "label": "steps", "type": "INT"},
            {"name": "cfg", "label": "cfg", "type": "FLOAT"},
        ],
        "outputs": [{"name": "value", "label": "value", "type": "FLOAT"}],
        "nodes": [
            {
                "id": 2,
                "type": "SamplerOptions",
                "inputs": [
                    {
                        "name": "seed",
                        "type": "INT",
                        "widget": {"name": "seed"},
                        "link": 10,
                    },
                    {
                        "name": "steps",
                        "type": "INT",
                        "widget": {"name": "steps"},
                        "link": 11,
                    },
                    {
                        "name": "cfg",
                        "type": "FLOAT",
                        "widget": {"name": "cfg"},
                        "link": 12,
                    },
                ],
                "outputs": [{"name": "value", "links": [13]}],
                "widgets_values": [123, "randomize", 28, 5.5],
                "properties": {"sugarcubes_symbol": "sampler"},
            }
        ],
        "links": [
            {"id": 10, "origin_id": -10, "origin_slot": 0, "target_id": 2},
            {"id": 11, "origin_id": -10, "origin_slot": 1, "target_id": 2},
            {"id": 12, "origin_id": -10, "origin_slot": 2, "target_id": 2},
            {
                "id": 13,
                "origin_id": 2,
                "origin_slot": 0,
                "target_id": -20,
                "target_slot": 0,
            },
        ],
    }
    document = cube_document(
        "Native defaults",
        nodes={"wrapper": {"class_type": "sampler-subgraph", "inputs": {}}},
        outputs={"output.value": ["wrapper", 0]},
        definitions={
            "SamplerOptions": {
                "input": {
                    "required": {
                        "seed": [
                            "INT",
                            {"default": 0, "control_after_generate": True},
                        ],
                        "steps": ["INT", {"default": 20}],
                        "cfg": ["FLOAT", {"default": 8.0}],
                    }
                },
                "output": ["FLOAT"],
            }
        },
        subgraphs=(sampler_subgraph,),
    )
    workflow = read_canonical_workflow(cube_workflow({"cube-a": document}))

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert _inputs(result.prompt["cube-a:wrapper:sampler"]) == {
        "seed": 123,
        "steps": 28,
        "cfg": 5.5,
    }


@pytest.mark.parametrize(
    ("outer_value", "expected"),
    [
        (None, "1x-default.pth"),
        ("4x-explicit.pth", "4x-explicit.pth"),
    ],
)
def test_nested_wrapper_uses_local_default_only_when_outer_value_is_unset(
    outer_value: object,
    expected: str,
) -> None:
    """Apply Cube override > nested node default without propagating null."""

    nested = {
        "id": "upscale-picker",
        "inputs": [{"name": "model_name", "type": "COMBO"}],
        "outputs": [{"name": "model", "type": "UPSCALE_MODEL"}],
        "nodes": [
            {
                "id": 10,
                "type": "UpscaleModelLoader",
                "inputs": [
                    {
                        "name": "model_name",
                        "type": "COMBO",
                        "widget": {"name": "model_name"},
                        "link": 1,
                    }
                ],
                "outputs": [{"name": "model", "links": [2]}],
                "widgets_values": ["1x-default.pth"],
                "properties": {"sugarcubes_symbol": "loader"},
            }
        ],
        "links": [
            {
                "id": 1,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 10,
                "target_slot": 0,
            },
            {
                "id": 2,
                "origin_id": 10,
                "origin_slot": 0,
                "target_id": -20,
                "target_slot": 0,
            },
        ],
    }
    cube = {
        "id": "cube-definition",
        "inputs": [],
        "outputs": [{"name": "model", "type": "UPSCALE_MODEL"}],
        "nodes": [
            {
                "id": 20,
                "type": "upscale-picker",
                "inputs": [
                    {
                        "name": "model_name",
                        "type": "COMBO",
                        "widget": {"name": "model_name"},
                        "link": None,
                    }
                ],
                "outputs": [{"name": "model", "links": [3]}],
                "widgets_values": [outer_value],
                "properties": {"sugarcubes_symbol": "wrapper"},
            }
        ],
        "links": [
            {
                "id": 3,
                "origin_id": 20,
                "origin_slot": 0,
                "target_id": -20,
                "target_slot": 0,
            }
        ],
    }
    definitions = {
        "upscale-picker": {
            "input": {"required": {"model_name": ["COMBO"]}},
            "input_order": {"required": ["model_name"]},
            "output": ["UPSCALE_MODEL"],
        },
        "UpscaleModelLoader": {
            "input": {"required": {"model_name": ["COMBO"]}},
            "input_order": {"required": ["model_name"]},
            "output": ["UPSCALE_MODEL"],
        },
    }

    prompt, _, _ = NativeSubgraphDocumentLowerer().lower_definition(
        "cube-a",
        None,
        cube,
        {},
        (nested,),
        definitions,
    )

    assert _inputs(prompt["cube-a:wrapper:loader"])["model_name"] == expected


def test_embedded_native_definition_owns_current_execution_values() -> None:
    """Execute the saved native Cube state instead of stale portable defaults."""

    definition = {
        "input": {
            "required": {
                "seed": ["INT", {"default": 0, "control_after_generate": True}],
                "steps": ["INT", {"default": 20}],
                "cfg": ["FLOAT", {"default": 8.0}],
            }
        },
        "output": ["FLOAT"],
    }
    document = cube_document(
        "Live native state",
        nodes={
            "sampler": {
                "class_type": "SamplerOptions",
                "inputs": {"seed": 0, "steps": 20, "cfg": 8.0},
            }
        },
        inputs={"seed": {"kind": "input", "targets": [["sampler", "seed"]]}},
        outputs={"output.value": ["sampler", 0]},
        definitions={"SamplerOptions": definition},
    )
    workflow_value = cube_workflow({"cube-a": document})
    definitions = workflow_value["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    native_definition = subgraphs[0]
    assert isinstance(native_definition, dict)
    native_definition.update(
        {
            "inputs": [{"name": "seed", "type": "INT"}],
            "outputs": [{"name": "value", "type": "FLOAT"}],
            "nodes": [
                {
                    "id": 7,
                    "type": "SamplerOptions",
                    "inputs": [
                        {
                            "name": "seed",
                            "widget": {"name": "seed"},
                            "link": 2,
                        },
                        {"name": "steps", "widget": {"name": "steps"}},
                        {"name": "cfg", "widget": {"name": "cfg"}},
                    ],
                    "outputs": [{"name": "value", "links": [1]}],
                    "widgets_values": [987, "increment", 42, 6.25],
                    "properties": {"sugarcubes_symbol": "sampler"},
                }
            ],
            "links": [
                {
                    "id": 2,
                    "origin_id": -10,
                    "origin_slot": 0,
                    "target_id": 7,
                    "target_slot": 0,
                },
                {
                    "id": 1,
                    "origin_id": 7,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                },
            ],
        }
    )
    root_node = workflow_value["nodes"]
    assert isinstance(root_node, list)
    assert isinstance(root_node[0], dict)
    root_node[0]["inputs"] = [{"name": "seed", "type": "INT", "link": None}]
    root_node[0]["outputs"] = [{"name": "value", "type": "FLOAT"}]
    workflow = read_canonical_workflow(workflow_value)

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert _inputs(result.prompt["cube-a:sampler"]) == {
        "seed": 987,
        "steps": 42,
        "cfg": 6.25,
    }
    assert [binding.binding for binding in result.boundary_bindings] == [
        "output.value",
        "seed",
        "value",
    ]


def test_native_saved_state_executes_without_a_portable_document() -> None:
    """Execute legacy native Cube state from its persisted graph and schemas."""

    node_definition = {"input": {}, "output": ["IMAGE"]}
    workflow_value = cube_workflow(
        {
            "cube-a": cube_document(
                "Native only",
                nodes={"stale": {"class_type": "StaleNode", "inputs": {}}},
                outputs={"output.image": ["stale", 0]},
            )
        }
    )
    definitions = workflow_value["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    native_definition = subgraphs[0]
    assert isinstance(native_definition, dict)
    native_definition.update(
        {
            "inputs": [],
            "outputs": [{"name": "image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": 7,
                    "type": "ImageSource",
                    "inputs": [],
                    "outputs": [{"name": "image", "links": [1]}],
                    "properties": {"sugarcubes_symbol": "source"},
                }
            ],
            "links": [
                {
                    "id": 1,
                    "origin_id": 7,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                }
            ],
        }
    )
    extra = native_definition["extra"]
    assert isinstance(extra, dict)
    extra.pop("sugarcubes_document")
    metadata = extra["sugarcubes_cube"]
    assert isinstance(metadata, dict)
    metadata["definitions"] = {"ImageSource": node_definition}
    root_nodes = workflow_value["nodes"]
    assert isinstance(root_nodes, list)
    root = root_nodes[0]
    assert isinstance(root, dict)
    root["inputs"] = []
    root["outputs"] = [{"name": "image", "type": "IMAGE"}]
    workflow = read_canonical_workflow(workflow_value)

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert result.prompt == {
        "cube-a:source": {"class_type": "ImageSource", "inputs": {}}
    }
    assert result.node_definitions == {"cube-a:source": node_definition}
    assert [
        (binding.binding, binding.node_id) for binding in result.boundary_bindings
    ] == [("image", "cube-a:source")]


def test_live_native_boundary_aliases_wire_connected_cubes() -> None:
    """Connect host boundary names to portable aliases after native flattening."""

    source = cube_document(
        "Source",
        nodes={"source": {"class_type": "ImageSource", "inputs": {}}},
        outputs={"output.image": ["source", 0]},
        definitions={"ImageSource": {"input": {}, "output": ["IMAGE"]}},
    )
    target = cube_document(
        "Target",
        nodes={"image": {"class_type": "ImagePass", "inputs": {"image": None}}},
        inputs={"input.value": {"kind": "input", "targets": [["image", "image"]]}},
        outputs={"output.image": ["image", 0]},
        definitions={
            "ImagePass": {
                "input": {"required": {"image": ["IMAGE"]}},
                "output": ["IMAGE"],
            }
        },
    )
    workflow_value = cube_workflow(
        {"cube-a": source, "cube-b": target},
        links=[[1, 1, 0, 2, 0, "IMAGE"]],
    )
    definitions = workflow_value["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    source_definition = subgraphs[0]
    target_definition = subgraphs[1]
    assert isinstance(source_definition, dict)
    assert isinstance(target_definition, dict)
    source_definition.update(
        {
            "inputs": [],
            "outputs": [{"name": "image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": 10,
                    "type": "ImageSource",
                    "inputs": [],
                    "outputs": [{"name": "image", "links": [100]}],
                    "properties": {"sugarcubes_symbol": "source"},
                }
            ],
            "links": [
                {
                    "id": 100,
                    "origin_id": 10,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                }
            ],
        }
    )
    target_definition.update(
        {
            "inputs": [{"name": "input.value", "type": "IMAGE"}],
            "outputs": [{"name": "image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": 20,
                    "type": "ImagePass",
                    "inputs": [{"name": "image", "link": 200}],
                    "outputs": [{"name": "image", "links": [201]}],
                    "properties": {"sugarcubes_symbol": "image"},
                }
            ],
            "links": [
                {
                    "id": 200,
                    "origin_id": -10,
                    "origin_slot": 0,
                    "target_id": 20,
                    "target_slot": 0,
                },
                {
                    "id": 201,
                    "origin_id": 20,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                },
            ],
        }
    )
    root_nodes = workflow_value["nodes"]
    assert isinstance(root_nodes, list)
    assert isinstance(root_nodes[0], dict)
    assert isinstance(root_nodes[1], dict)
    root_nodes[0]["inputs"] = []
    root_nodes[0]["outputs"] = [{"name": "image", "type": "IMAGE"}]
    root_nodes[1]["inputs"] = [{"name": "input.value", "type": "IMAGE"}]
    root_nodes[1]["outputs"] = [{"name": "image", "type": "IMAGE"}]
    workflow = read_canonical_workflow(workflow_value)

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert _inputs(result.prompt["cube-b:image"])["image"] == [
        "cube-a:source",
        0,
    ]


def test_live_boundary_wiring_reaches_nodes_inside_nested_native_wrappers() -> None:
    """Replace public markers after recursively flattening a wrapper fanout."""

    nested = {
        "id": "nested-wrapper",
        "inputs": [{"name": "image", "label": "image", "type": "IMAGE"}],
        "outputs": [{"name": "image", "label": "image", "type": "IMAGE"}],
        "nodes": [
            {
                "id": 30,
                "type": "ImagePass",
                "inputs": [{"name": "image", "link": 300}],
                "outputs": [{"name": "image", "links": [301]}],
                "properties": {"sugarcubes_symbol": "inner"},
            }
        ],
        "links": [
            {
                "id": 300,
                "origin_id": -10,
                "origin_slot": 0,
                "target_id": 30,
                "target_slot": 0,
            },
            {
                "id": 301,
                "origin_id": 30,
                "origin_slot": 0,
                "target_id": -20,
                "target_slot": 0,
            },
        ],
    }
    source = cube_document(
        "Source",
        nodes={"source": {"class_type": "ImageSource", "inputs": {}}},
        outputs={"output.image": ["source", 0]},
        definitions={"ImageSource": {"input": {}, "output": ["IMAGE"]}},
    )
    target = cube_document(
        "Nested target",
        nodes={
            "wrapper": {
                "class_type": "nested-wrapper",
                "inputs": {"image": ["@binding", "input.value"]},
            }
        },
        inputs={
            "input.value": {
                "kind": "input",
                "targets": [["wrapper", "image"]],
            }
        },
        outputs={"output.image": ["wrapper", 0]},
        definitions={
            "ImagePass": {
                "input": {"required": {"image": ["IMAGE"]}},
                "output": ["IMAGE"],
            }
        },
        subgraphs=(nested,),
    )
    workflow_value = cube_workflow(
        {"cube-a": source, "cube-b": target},
        links=[[1, 1, 0, 2, 0, "IMAGE"]],
    )
    definitions = workflow_value["definitions"]
    assert isinstance(definitions, dict)
    subgraphs = definitions["subgraphs"]
    assert isinstance(subgraphs, list)
    source_definition = subgraphs[0]
    target_definition = subgraphs[1]
    assert isinstance(source_definition, dict)
    assert isinstance(target_definition, dict)
    source_definition.update(
        {
            "inputs": [],
            "outputs": [{"name": "image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": 10,
                    "type": "ImageSource",
                    "inputs": [],
                    "outputs": [{"name": "image", "links": [100]}],
                    "properties": {"sugarcubes_symbol": "source"},
                }
            ],
            "links": [
                {
                    "id": 100,
                    "origin_id": 10,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                }
            ],
        }
    )
    target_definition.update(
        {
            "inputs": [{"name": "input.value", "type": "IMAGE"}],
            "outputs": [{"name": "image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": 20,
                    "type": "nested-wrapper",
                    "inputs": [{"name": "image", "link": 200}],
                    "outputs": [{"name": "image", "links": [201]}],
                    "properties": {"sugarcubes_symbol": "wrapper"},
                }
            ],
            "links": [
                {
                    "id": 200,
                    "origin_id": -10,
                    "origin_slot": 0,
                    "target_id": 20,
                    "target_slot": 0,
                },
                {
                    "id": 201,
                    "origin_id": 20,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                },
            ],
        }
    )
    root_nodes = workflow_value["nodes"]
    assert isinstance(root_nodes, list)
    assert isinstance(root_nodes[0], dict)
    assert isinstance(root_nodes[1], dict)
    root_nodes[0]["inputs"] = []
    root_nodes[0]["outputs"] = [{"name": "image", "type": "IMAGE"}]
    root_nodes[1]["inputs"] = [{"name": "input.value", "type": "IMAGE"}]
    root_nodes[1]["outputs"] = [{"name": "image", "type": "IMAGE"}]
    workflow = read_canonical_workflow(workflow_value)

    result = NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert _inputs(result.prompt["cube-b:wrapper:inner"])["image"] == [
        "cube-a:source",
        0,
    ]


def test_rejects_recursive_native_subgraph_definition_cycles() -> None:
    """Fail closed on definition recursion instead of exhausting Python."""

    def recursive_subgraph(identity: str, child: str) -> dict[str, object]:
        """Build one wrapper definition that delegates to another definition."""

        return {
            "id": identity,
            "inputs": [{"name": "image", "label": "image", "type": "IMAGE"}],
            "outputs": [{"name": "image", "label": "image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": 1,
                    "type": child,
                    "inputs": [{"name": "image", "link": 1}],
                    "outputs": [{"name": "image", "links": [2]}],
                }
            ],
            "links": [
                {
                    "id": 1,
                    "origin_id": -10,
                    "origin_slot": 0,
                    "target_id": 1,
                    "target_slot": 0,
                },
                {
                    "id": 2,
                    "origin_id": 1,
                    "origin_slot": 0,
                    "target_id": -20,
                    "target_slot": 0,
                },
            ],
        }

    document = cube_document(
        "Recursive",
        nodes={
            "wrapper": {
                "class_type": "recursive-a",
                "inputs": {"image": ["@binding", "input.image"]},
            }
        },
        inputs={"input.image": {"kind": "input", "targets": [["wrapper", "image"]]}},
        outputs={"output.image": ["wrapper", 0]},
        subgraphs=(
            recursive_subgraph("recursive-a", "recursive-b"),
            recursive_subgraph("recursive-b", "recursive-a"),
        ),
    )
    workflow = read_canonical_workflow(cube_workflow({"cube-a": document}))

    with pytest.raises(CubeLoweringError) as captured:
        NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert captured.value.code == "execution.lowering.subgraph_definition_cycle"


def test_rejects_native_scopes_above_the_node_limit() -> None:
    """Bound embedded native breadth before building execution indexes."""

    oversized = {
        "id": "oversized",
        "inputs": [],
        "outputs": [],
        "nodes": [{}] * (MAX_NATIVE_SCOPE_NODES + 1),
        "links": [],
    }
    document = cube_document(
        "Oversized",
        nodes={"wrapper": {"class_type": "oversized", "inputs": {}}},
        inputs={},
        outputs={},
        subgraphs=(oversized,),
    )
    workflow = read_canonical_workflow(cube_workflow({"cube-a": document}))

    with pytest.raises(CubeLoweringError) as captured:
        NativeCubeWorkflowLowerer().lower(workflow, build_cube_topology(workflow))

    assert captured.value.code == "execution.lowering.subgraph_limit_exceeded"


def _inputs(node: Mapping[str, object]) -> Mapping[str, object]:
    """Narrow one lowered prompt input object for contract assertions."""

    inputs = node.get("inputs")
    assert isinstance(inputs, Mapping)
    return inputs
