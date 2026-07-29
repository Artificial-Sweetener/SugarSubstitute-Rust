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
"""Verify export-only projection of native Cube subgraph boundaries."""

from __future__ import annotations

from typing import Any

from sugarcubes.exporter import export
from sugarcubes.exporter.graph import analyze_cubes, build_graph
from sugarcubes.exporter.native_subgraph_adapter import (
    project_native_cube_exports,
)


def _workflow() -> dict[str, Any]:
    """Return one native Cube workflow with graph-owned input and output."""

    definition_id = "11111111-1111-4111-8111-111111111111"
    return {
        "version": 1,
        "nodes": [
            {
                "id": 7,
                "type": definition_id,
                "title": "Detailer",
                "pos": [100, 120],
                "size": [800, 500],
                "properties": {
                    "sugarcubes_cube": {
                        "cube_id": "local/personal/Detailer.cube",
                        "default_alias": "Detailer",
                    }
                },
            }
        ],
        "links": [],
        "groups": [],
        "definitions": {
            "subgraphs": [
                {
                    "id": definition_id,
                    "name": "Cube: Detailer",
                    "inputNode": {"id": -10, "bounding": [-120, 20, 75, 100]},
                    "outputNode": {"id": -20, "bounding": [420, 20, 75, 100]},
                    "inputs": [
                        {
                            "id": "input-image",
                            "name": "image",
                            "type": "IMAGE",
                            "linkIds": [1],
                        }
                    ],
                    "outputs": [
                        {
                            "id": "output-image",
                            "name": "image",
                            "type": "IMAGE",
                            "linkIds": [2],
                        }
                    ],
                    "widgets": [],
                    "nodes": [
                        {
                            "id": "inner",
                            "type": "ImageScale",
                            "title": "Scale",
                            "pos": [0, 20],
                            "size": [260, 180],
                            "inputs": [
                                {
                                    "name": "image",
                                    "type": "IMAGE",
                                    "link": 1,
                                }
                            ],
                            "outputs": [
                                {
                                    "name": "IMAGE",
                                    "type": "IMAGE",
                                    "links": [2],
                                }
                            ],
                            "widgets_values": ["nearest-exact", 1.0],
                        }
                    ],
                    "links": [
                        {
                            "id": 1,
                            "origin_id": -10,
                            "origin_slot": 0,
                            "target_id": "inner",
                            "target_slot": 0,
                            "type": "IMAGE",
                        },
                        {
                            "id": 2,
                            "origin_id": "inner",
                            "origin_slot": 0,
                            "target_id": -20,
                            "target_slot": 0,
                            "type": "IMAGE",
                        },
                    ],
                    "groups": [],
                    "reroutes": [],
                }
            ]
        },
    }


def test_projection_builds_export_boundaries_without_mutating_live_payloads() -> None:
    """Native interfaces should reuse the existing stable serializer contract."""

    graph: dict[str, Any] = {
        "7:inner": {
            "class_type": "ImageScale",
            "inputs": {"upscale_method": "nearest-exact", "scale_by": 1.0},
            "_meta": {"title": "Scale"},
        }
    }
    workflow = _workflow()
    entries = {
        "local/personal/Detailer.cube": {
            "definition_id": "11111111-1111-4111-8111-111111111111",
            "instance_node_ids": ["7"],
            "metadata": {"default_alias": "Detailer"},
        }
    }

    projected_graph, projected_workflow = project_native_cube_exports(
        graph, workflow, entries
    )
    analysis = analyze_cubes(projected_graph, workflow=projected_workflow)
    cube = analysis.cubes["local/personal/Detailer.cube"]

    assert graph["7:inner"]["inputs"].get("image") is None
    assert len(workflow["nodes"]) == 1
    assert cube.subgraph_nodes == {"7:inner"}
    assert [marker.kind for marker in cube.inputs] == ["input"]
    assert [marker.kind for marker in cube.outputs] == ["output"]
    projected_inner = analysis.graph.nodes["7:inner"]
    assert list(analysis.graph.edges_to(projected_inner.id))[0].source.startswith(
        "sugarcubes:native:"
    )
    assert list(analysis.graph.edges_from(projected_inner.id))[0].target.startswith(
        "sugarcubes:native:"
    )


def test_projection_uses_non_node_container_identity_without_root_wrapper() -> None:
    """Surface containers should identify definitions without serialized graph nodes."""

    definition_id = "11111111-1111-4111-8111-111111111111"
    workflow = _workflow()
    workflow["nodes"] = []
    workflow["extra"] = {
        "sugarcubes_containers": {
            "schema": 1,
            "items": [
                {
                    "id": "instance-1",
                    "definition_id": definition_id,
                    "title": "Detailer",
                    "pos": [100, 120],
                    "size": [800, 500],
                    "identity": {
                        "cube_id": "local/personal/Detailer.cube",
                    },
                    "surface": {},
                }
            ],
            "links": [],
        }
    }
    graph: dict[str, Any] = {
        "instance-1:inner": {
            "class_type": "ImageScale",
            "inputs": {"upscale_method": "nearest-exact", "scale_by": 1.0},
            "_meta": {"title": "Scale"},
        }
    }
    entries = {
        "local/personal/Detailer.cube": {
            "definition_id": definition_id,
            "instance_container_ids": ["instance-1"],
            "metadata": {"default_alias": "Detailer"},
        }
    }

    projected_graph, projected_workflow = project_native_cube_exports(
        graph, workflow, entries
    )
    analysis = analyze_cubes(projected_graph, workflow=projected_workflow)

    assert workflow["nodes"] == []
    assert analysis.cubes["local/personal/Detailer.cube"].subgraph_nodes == {
        "instance-1:inner"
    }


def test_projection_exports_a_blank_native_cube_without_public_ports() -> None:
    """A new empty Cube must save without serializing synthetic interface ports."""

    definition_id = "11111111-1111-4111-8111-111111111111"
    workflow = _workflow()
    definition = workflow["definitions"]["subgraphs"][0]
    definition["nodes"] = []
    definition["links"] = []
    definition["inputs"] = []
    definition["outputs"] = []
    graph: dict[str, Any] = {}
    entries = {
        "local/personal/Blank.cube": {
            "definition_id": definition_id,
            "instance_node_ids": ["7"],
            "metadata": {"default_alias": "Blank"},
        }
    }

    projected_graph, projected_workflow = project_native_cube_exports(
        graph, workflow, entries
    )
    analysis = analyze_cubes(projected_graph, workflow=projected_workflow)
    exported = export(
        projected_graph,
        workflow=projected_workflow,
        cube_ids=["local/personal/Blank.cube"],
    )

    cube = analysis.cubes["local/personal/Blank.cube"]
    assert cube.inputs == []
    assert cube.outputs == []
    assert [anchor.kind for anchor in cube.anchors] == ["anchor"]
    assert cube.subgraph_nodes == set()
    assert len(exported) == 1
    assert exported[0].cube.get("inputs", {}) == {}
    assert exported[0].cube.get("outputs", {}) == {}


def test_build_graph_accepts_comfy_flattened_execution_ids() -> None:
    """Colon-delimited native subgraph execution ids are real prompt nodes."""

    graph = build_graph(
        {
            "7:inner": {
                "class_type": "ImageScale",
                "inputs": {"scale_by": 2.0},
            },
            "workflow": {"metadata": True},
        }
    )

    assert set(graph.nodes) == {"7:inner"}


def test_projection_remaps_nested_subgraph_workflow_links() -> None:
    """Nested native subgraphs should retain valid root-projection link ownership."""

    workflow = _workflow()
    definition = workflow["definitions"]["subgraphs"][0]
    definition["nodes"] = [
        {
            "id": "nested",
            "type": "22222222-2222-4222-8222-222222222222",
            "title": "Nested Subgraph",
            "pos": [0, 20],
            "size": [260, 180],
            "inputs": [{"name": "image", "type": "IMAGE", "link": 1}],
            "outputs": [{"name": "IMAGE", "type": "IMAGE", "links": [2]}],
        }
    ]
    definition["links"][0]["target_id"] = "nested"
    definition["links"][1]["origin_id"] = "nested"
    graph: dict[str, Any] = {
        "7:nested": {
            "class_type": "22222222-2222-4222-8222-222222222222",
            "inputs": {},
            "_meta": {"title": "Nested Subgraph"},
        }
    }
    entries = {
        "local/personal/Detailer.cube": {
            "definition_id": "11111111-1111-4111-8111-111111111111",
            "instance_node_ids": ["7"],
            "metadata": {"default_alias": "Detailer"},
        }
    }

    _, projected_workflow = project_native_cube_exports(graph, workflow, entries)

    nested = next(
        node for node in projected_workflow["nodes"] if node["id"] == "7:nested"
    )
    input_link = nested["inputs"][0]["link"]
    output_links = nested["outputs"][0]["links"]
    incoming = next(
        link for link in projected_workflow["links"] if link[3] == "7:nested"
    )
    outgoing = next(
        link for link in projected_workflow["links"] if link[1] == "7:nested"
    )
    assert input_link == incoming[0]
    assert output_links == [outgoing[0]]


def test_projection_accepts_a_cube_wrapper_inside_a_parent_subgraph() -> None:
    """A Cube authored while editing another Cube still has a serializable native wrapper."""

    parent_id = "11111111-1111-4111-8111-111111111111"
    child_id = "22222222-2222-4222-8222-222222222222"
    workflow = _workflow()
    parent = workflow["definitions"]["subgraphs"][0]
    parent["id"] = parent_id
    parent["nodes"] = [{"id": 21, "type": child_id}]
    workflow["definitions"]["subgraphs"].append(
        {
            "id": child_id,
            "name": "Cube: Nested Detailer",
            "inputNode": {"id": -10, "bounding": [-120, 20, 75, 100]},
            "outputNode": {"id": -20, "bounding": [420, 20, 75, 100]},
            "inputs": [{"name": "image", "type": "IMAGE"}],
            "outputs": [{"name": "image", "type": "IMAGE"}],
            "nodes": [
                {
                    "id": "inner",
                    "type": "ImageScale",
                    "pos": [0, 20],
                    "size": [260, 180],
                    "inputs": [{"name": "image", "type": "IMAGE", "link": 1}],
                    "outputs": [{"name": "IMAGE", "type": "IMAGE", "links": [2]}],
                }
            ],
            "links": [
                [1, -10, 0, "inner", 0, "IMAGE"],
                [2, "inner", 0, -20, 0, "IMAGE"],
            ],
        }
    )
    graph = {
        "21:inner": {
            "class_type": "ImageScale",
            "inputs": {"scale_by": 1.0},
            "_meta": {"title": "Scale"},
        }
    }
    entries = {
        "local/personal/Flux/Nested Detailer.cube": {
            "definition_id": child_id,
            "instance_node_ids": ["21"],
            "metadata": {"default_alias": "Flux/Nested Detailer"},
        }
    }

    projected_graph, projected_workflow = project_native_cube_exports(
        graph, workflow, entries
    )
    analysis = analyze_cubes(projected_graph, workflow=projected_workflow)

    assert analysis.cubes[
        "local/personal/Flux/Nested Detailer.cube"
    ].subgraph_nodes == {"21:inner"}
