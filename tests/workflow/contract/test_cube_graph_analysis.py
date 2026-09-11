#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Specify headless Cube projection without depending on Comfy frontend objects."""

from __future__ import annotations

from typing import cast

from sugarcubes.workflow_analysis import analyze_cube_graph
from tests.execution.support.execution_fixtures import cube_document, cube_workflow


def test_analysis_derives_ordered_typed_proximity_from_native_cube_geometry() -> None:
    """Match adjacent free Cube boundaries using the frontend-owned contract."""

    workflow = _positioned_cube_pair()

    analysis = analyze_cube_graph(workflow)

    assert [edge.semantic_key for edge in analysis.edges] == [
        ("source", "output.image", "target", "input.image"),
        ("source", "output.mask", "target", "input.mask"),
    ]
    assert [edge.origin.value for edge in analysis.edges] == [
        "proximity",
        "proximity",
    ]
    assert analysis.segments[0].instance_ids == ("source", "target")
    assert analysis.segments[0].reorderable is True
    assert analysis.normalized_workflow["extra"] == {"fixture": True}


def test_analysis_matches_same_typed_ports_when_input_declaration_order_differs() -> (
    None
):
    """Never pair IMAGE and MASK boundaries by raw cross-type slot position."""

    workflow = _positioned_cube_pair()
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    nodes[1]["inputs"] = [
        {"name": "input.mask", "type": "MASK", "link": None},
        {"name": "input.image", "type": "IMAGE", "link": None},
    ]

    analysis = analyze_cube_graph(workflow)

    assert [edge.semantic_key for edge in analysis.edges] == [
        ("source", "output.image", "target", "input.image"),
        ("source", "output.mask", "target", "input.mask"),
    ]


def test_analysis_preserves_manual_links_and_leaves_surplus_ports_free() -> None:
    """Exclude manually occupied slots without inventing replacement edges."""

    workflow = _positioned_cube_pair()
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    source_outputs = cast(list[dict[str, object]], nodes[0]["outputs"])
    target_inputs = cast(list[dict[str, object]], nodes[1]["inputs"])
    source_outputs[0]["links"] = [41]
    target_inputs[1]["link"] = 41
    workflow["links"] = [[41, 1, 0, 2, 1, "IMAGE"]]

    analysis = analyze_cube_graph(workflow)

    assert [edge.semantic_key for edge in analysis.edges] == [
        ("source", "output.image", "target", "input.mask"),
    ]
    assert analysis.edges[0].origin.value == "explicit"


def test_analysis_returns_no_stack_segments_for_an_ordinary_workflow() -> None:
    """Keep ordinary Comfy workflows opaque and free of Cube UI projection."""

    analysis = analyze_cube_graph(
        {
            "version": 0.4,
            "nodes": [{"id": 1, "type": "LoadImage", "pos": [0, 0]}],
            "links": [],
            "definitions": {"subgraphs": []},
            "extra": {"preserve": {"unknown": True}},
        }
    )

    assert analysis.instances == ()
    assert analysis.edges == ()
    assert analysis.segments == ()


def test_analysis_orders_disconnected_cube_segments_by_native_geometry() -> None:
    """Project separate Cube regions in their authored left-to-right order."""

    workflow = _positioned_cube_pair()
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    nodes[0]["properties"]["sugarcubes_cube"]["instance_id"] = "zeta"  # type: ignore[index]
    nodes[0]["properties"]["sugarcubes_cube"]["instance_alias"] = "Zeta"  # type: ignore[index]
    nodes[1]["properties"]["sugarcubes_cube"]["instance_id"] = "alpha"  # type: ignore[index]
    nodes[1]["properties"]["sugarcubes_cube"]["instance_alias"] = "Alpha"  # type: ignore[index]
    nodes[1]["pos"] = [500, 0]

    analysis = analyze_cube_graph(workflow)

    assert [segment.instance_ids for segment in analysis.segments] == [
        ("zeta",),
        ("alpha",),
    ]


def test_analysis_keeps_adjacent_asymmetric_cubes_in_one_reorderable_segment() -> None:
    """Keep a visual Cube series rearrangeable when one order has dangling ports."""

    workflow = _positioned_cube_pair()
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    nodes.reverse()
    nodes[0]["pos"] = [0, 0]
    nodes[1]["pos"] = [220, 0]

    analysis = analyze_cube_graph(workflow)

    assert analysis.edges == ()
    assert [segment.instance_ids for segment in analysis.segments] == [
        ("target", "source"),
    ]
    assert analysis.segments[0].reorderable is True


def test_analysis_treats_ordinary_graph_region_as_fixed_cube_boundaries() -> None:
    """Expose ordinary nodes as opaque separators without tracing through them."""

    workflow = _positioned_cube_pair()
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    nodes[1]["pos"] = [600, 0]
    nodes.insert(
        1,
        {
            "id": "ordinary-boundary",
            "type": "VendorOpaqueNode",
            "pos": [250, 0],
            "properties": {"vendor": {"unknown": [1, 2, 3]}},
        },
    )
    workflow["links"] = [
        [41, 1, 0, "ordinary-boundary", 0, "IMAGE"],
        [42, "ordinary-boundary", 0, 2, 0, "IMAGE"],
    ]

    analysis = analyze_cube_graph(workflow)

    assert [segment.instance_ids for segment in analysis.segments] == [
        ("source",),
        ("target",),
    ]
    assert [segment.reorderable for segment in analysis.segments] == [False, False]
    assert [segment.boundary_node_ids for segment in analysis.segments] == [
        ("ordinary-boundary",),
        ("ordinary-boundary",),
    ]
    assert analysis.normalized_workflow["nodes"][1] == nodes[1]  # type: ignore[index]


def _positioned_cube_pair() -> dict[str, object]:
    """Build two native Cubes matching the real Anima boundary shape."""

    source = cube_document(
        "Source",
        nodes={"relay": {"class_type": "Relay", "inputs": {}}},
        outputs={"output.image": ["relay", 0], "output.mask": ["relay", 1]},
        definitions={"Relay": {"input": {"required": {}}, "output": ["IMAGE", "MASK"]}},
    )
    target = cube_document(
        "Target",
        nodes={
            "relay": {
                "class_type": "Target",
                "inputs": {"image": None, "mask": None},
            }
        },
        inputs={
            "input.image": {"kind": "input", "targets": [["relay", "image"]]},
            "input.mask": {"kind": "input", "targets": [["relay", "mask"]]},
        },
        definitions={
            "Target": {
                "input": {"required": {"image": ["IMAGE"], "mask": ["MASK"]}},
                "output": ["IMAGE"],
            }
        },
    )
    workflow = cube_workflow({"source": source, "target": target})
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    nodes[0]["pos"] = [0, 0]
    nodes[0]["size"] = [100, 200]
    nodes[0]["outputs"] = [
        {"name": "output.image", "type": "IMAGE", "links": None},
        {"name": "output.mask", "type": "MASK", "links": None},
    ]
    nodes[1]["pos"] = [220, 0]
    nodes[1]["size"] = [100, 200]
    nodes[1]["inputs"] = [
        {"name": "input.image", "type": "IMAGE", "link": None},
        {"name": "input.mask", "type": "MASK", "link": None},
        {"name": "input.extra", "type": "IMAGE", "link": None},
    ]
    nodes[0]["inputs"] = []
    nodes[1]["outputs"] = []
    return workflow
