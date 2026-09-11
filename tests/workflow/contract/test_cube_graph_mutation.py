#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
"""Verify SugarCubes-owned atomic Cube graph reordering."""

from __future__ import annotations

from copy import deepcopy
from typing import cast

import pytest

from sugarcubes.workflow_analysis import CubeGraphAnalysisService
from sugarcubes.workflow_mutation import (
    CubeGraphDraft,
    CubeGraphMutationError,
    CubeGraphMutationService,
)
from tests.execution.support.execution_fixtures import (
    cube_document,
    cube_workflow,
    image_passthrough_document,
)


def test_reorders_only_proximity_cube_geometry_and_preserves_ordinary_data() -> None:
    """Match native Cube swapping without rewriting host-owned graph content."""

    workflow = cube_workflow(
        {
            "first": image_passthrough_document("First"),
            "second": image_passthrough_document("Second"),
            "third": image_passthrough_document("Third"),
        },
        loose_nodes=(
            {
                "id": "ordinary",
                "type": "ThirdPartyNode",
                "pos": [900, 600],
                "size": [120, 80],
                "properties": {"unknown": {"preserve": True}},
            },
        ),
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for index, node in enumerate(nodes[:3]):
        node["pos"] = [index * 124, 40]
        node["size"] = [100, 200]
    before_ordinary = deepcopy(nodes[-1])
    before_links = deepcopy(workflow["links"])
    service = CubeGraphMutationService(CubeGraphAnalysisService())

    result = service.reorder(
        workflow,
        segment_instance_ids=("first", "second", "third"),
        ordered_instance_ids=("third", "first", "second"),
    )

    assert result.segments[0].instance_ids == ("third", "first", "second")
    assert result.normalized_workflow["links"] == before_links
    mutated_nodes = cast(list[dict[str, object]], result.normalized_workflow["nodes"])
    assert mutated_nodes[-1] == before_ordinary
    assert workflow["nodes"] == nodes


def test_reorders_asymmetric_cube_series_with_dangling_reverse_boundary() -> None:
    """Allow either visual order even when only one direction has compatible ports."""

    source = cube_document(
        "Prompt by Region",
        nodes={"source": {"class_type": "Source", "inputs": {}}},
        outputs={
            "output.image": ["source", 0],
            "output.mask": ["source", 1],
        },
        definitions={
            "Source": {"input": {"required": {}}, "output": ["IMAGE", "MASK"]}
        },
    )
    target = cube_document(
        "Diffusion Upscale",
        nodes={
            "target": {
                "class_type": "Target",
                "inputs": {"image": None, "mask": None},
            }
        },
        inputs={
            "input.image": {"kind": "input", "targets": [["target", "image"]]},
            "input.mask": {"kind": "input", "targets": [["target", "mask"]]},
        },
        definitions={
            "Target": {
                "input": {"required": {"image": ["IMAGE"], "mask": ["MASK"]}},
                "output": ["IMAGE"],
            }
        },
    )
    workflow = cube_workflow({"prompt": source, "upscale": target})
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for index, node in enumerate(nodes):
        node["pos"] = [index * 220, 0]
        node["size"] = [100, 200]
    nodes[0]["inputs"] = []
    nodes[0]["outputs"] = [
        {"name": "output.image", "type": "IMAGE", "links": None},
        {"name": "output.mask", "type": "MASK", "links": None},
    ]
    nodes[1]["inputs"] = [
        {"name": "input.image", "type": "IMAGE", "link": None},
        {"name": "input.mask", "type": "MASK", "link": None},
    ]
    nodes[1]["outputs"] = []
    service = CubeGraphMutationService(CubeGraphAnalysisService())

    reversed_result = service.reorder(
        workflow,
        segment_instance_ids=("prompt", "upscale"),
        ordered_instance_ids=("upscale", "prompt"),
    )

    assert reversed_result.edges == ()
    assert reversed_result.segments[0].instance_ids == ("upscale", "prompt")
    assert reversed_result.segments[0].reorderable is True

    restored_result = service.reorder(
        reversed_result.normalized_workflow,
        segment_instance_ids=("upscale", "prompt"),
        ordered_instance_ids=("prompt", "upscale"),
    )

    assert [edge.semantic_key for edge in restored_result.edges] == [
        ("prompt", "output.image", "upscale", "input.image"),
        ("prompt", "output.mask", "upscale", "input.mask"),
    ]


def test_explicitly_wired_cube_segment_is_not_automatically_reordered() -> None:
    """Preserve manual Cube wiring as an authoritative fixed graph segment."""

    workflow = cube_workflow(
        {
            "first": image_passthrough_document("First"),
            "second": image_passthrough_document("Second"),
        },
        links=([1, 1, 0, 2, 0, "IMAGE"],),
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    for index, node in enumerate(nodes):
        node["pos"] = [index * 124, 40]
        node["size"] = [100, 200]
    service = CubeGraphMutationService(CubeGraphAnalysisService())

    with pytest.raises(CubeGraphMutationError, match="explicit wiring"):
        service.reorder(
            workflow,
            segment_instance_ids=("first", "second"),
            ordered_instance_ids=("second", "first"),
        )


def test_append_cube_preserves_ordinary_graph_and_uses_exact_cube_ports() -> None:
    """Insert one Cube without interpreting any ordinary Comfy graph record."""

    workflow: dict[str, object] = {
        "version": 0.4,
        "nodes": [
            {
                "id": "ordinary",
                "type": "ThirdPartyNode",
                "pos": [701, -33],
                "properties": {"vendor": {"opaque": [1, 2, 3]}},
            }
        ],
        "links": [[9, "external", 0, "ordinary", 1, "VENDOR"]],
        "definitions": {"subgraphs": []},
        "extra": {"vendor": {"preserve": True}},
    }
    original = deepcopy(workflow)
    service = CubeGraphMutationService(CubeGraphAnalysisService())

    result = service.append_cube(
        workflow,
        instance_id="added",
        alias="Added",
        bypassed=False,
        document=image_passthrough_document("Added"),
    )

    assert workflow == original
    nodes = cast(list[dict[str, object]], result.normalized_workflow["nodes"])
    original_nodes = cast(list[dict[str, object]], original["nodes"])
    assert nodes[0] == original_nodes[0]
    assert result.normalized_workflow["links"] == original["links"]
    assert result.normalized_workflow["extra"] == original["extra"]
    assert nodes[1]["inputs"] == [{"name": "input.image", "type": "IMAGE"}]
    assert nodes[1]["outputs"] == [{"name": "output.image", "type": "IMAGE"}]
    assert result.instances[0].instance_id == "added"


def test_remove_cube_preserves_ordinary_nodes_and_unrelated_links() -> None:
    """Remove exactly one recognized Cube and links touching its root node."""

    workflow = cube_workflow(
        {"cube": image_passthrough_document("Cube")},
        loose_nodes=(
            {"id": "left", "type": "Left", "properties": {"opaque": "left"}},
            {"id": "right", "type": "Right", "properties": {"opaque": "right"}},
        ),
    )
    nodes = cast(list[dict[str, object]], workflow["nodes"])
    cube_node_id = nodes[0]["id"]
    workflow["links"] = [
        [1, "left", 0, "right", 0, "IMAGE"],
        [2, "left", 0, cube_node_id, 0, "IMAGE"],
        [3, cube_node_id, 0, "right", 0, "IMAGE"],
    ]
    ordinary_nodes = deepcopy(nodes[1:])
    service = CubeGraphMutationService(CubeGraphAnalysisService())

    result = service.remove_cube(workflow, instance_id="cube")

    assert result.instances == ()
    assert result.normalized_workflow["nodes"] == ordinary_nodes
    assert result.normalized_workflow["links"] == [[1, "left", 0, "right", 0, "IMAGE"]]
    assert result.normalized_workflow["extra"] == {"fixture": True}


def test_remove_cube_rejects_non_cube_identity() -> None:
    """Never permit the Cube mutation API to target an ordinary Comfy node."""

    workflow = {
        "version": 0.4,
        "nodes": [{"id": "ordinary", "type": "Vendor", "properties": {}}],
        "links": [],
        "definitions": {"subgraphs": []},
    }

    with pytest.raises(CubeGraphMutationError, match="Cube instance is unavailable"):
        CubeGraphMutationService(CubeGraphAnalysisService()).remove_cube(
            workflow,
            instance_id="ordinary",
        )


def test_create_cube_workflow_builds_ordered_native_graph_in_one_analysis() -> None:
    """Migrate an ordered legacy stack without per-Cube analysis passes."""

    class _CountingAnalysis(CubeGraphAnalysisService):
        """Record complete graph-analysis passes."""

        def __init__(self) -> None:
            super().__init__()
            self.calls = 0

        def analyze(self, workflow_value: object):  # type: ignore[no-untyped-def]
            """Count and delegate one complete analysis."""

            self.calls += 1
            return super().analyze(workflow_value)

    analysis = _CountingAnalysis()
    identifiers = iter(("node-a", "definition-a", "node-b", "definition-b"))
    service = CubeGraphMutationService(analysis, lambda: next(identifiers))

    result = service.create_cube_workflow(
        (
            CubeGraphDraft(
                instance_id="first",
                alias="First",
                bypassed=False,
                document=image_passthrough_document("First"),
            ),
            CubeGraphDraft(
                instance_id="second",
                alias="Second",
                bypassed=True,
                document=image_passthrough_document("Second"),
            ),
        )
    )

    assert analysis.calls == 1
    assert [instance.instance_id for instance in result.instances] == [
        "first",
        "second",
    ]
    assert result.segments[0].instance_ids == ("first", "second")
    nodes = cast(list[dict[str, object]], result.normalized_workflow["nodes"])
    assert [node["mode"] for node in nodes] == [0, 4]
