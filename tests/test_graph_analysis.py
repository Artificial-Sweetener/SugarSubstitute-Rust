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
import pytest

from sugarcubes.exporter import export as export_cubes
from sugarcubes.exporter.graph import analyze_cubes, build_graph


def test_build_graph_ignores_non_numeric_keys():
    prompt = {
        "workflow": {"nodes": []},
        "1": {"class_type": "KSampler", "inputs": {}},
    }
    graph = build_graph(prompt)

    assert "1" in graph.nodes
    assert "workflow" not in graph.nodes


def test_analyze_cubes_requires_default_alias():
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": "local/example-user/demo.cube"},
        },
        "2": {"class_type": "KSampler", "inputs": {"image": ["1", 0]}},
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": "local/example-user/demo.cube",
                "default_alias": "Demo",
                "value": ["2", 0],
            },
        },
    }

    with pytest.raises(ValueError, match="missing default_alias"):
        analyze_cubes(prompt)


def test_analyze_cubes_requires_cube_id():
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"default_alias": "Demo"},
        },
        "2": {"class_type": "KSampler", "inputs": {"image": ["1", 0]}},
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"default_alias": "Demo", "value": ["2", 0]},
        },
    }

    with pytest.raises(ValueError, match="missing cube_id"):
        analyze_cubes(prompt)


def test_analyze_cubes_allows_mismatched_default_aliass():
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Alpha"},
        },
        "2": {"class_type": "KSampler", "inputs": {"image": ["1", 0]}},
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Beta", "value": ["2", 0]},
        },
    }

    analysis = analyze_cubes(prompt)

    assert analysis.cubes[cube_id].name == "Alpha"


def test_analyze_cubes_prefers_lookup_when_live_name_matches_cube_id():
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": cube_id},
        },
        "2": {"class_type": "KSampler", "inputs": {"image": ["1", 0]}},
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": cube_id, "value": ["2", 0]},
        },
    }

    analysis = analyze_cubes(prompt, default_alias_lookup={cube_id: "Canonical Name"})

    assert analysis.cubes[cube_id].name == "Canonical Name"


def test_analyze_cubes_preserves_human_live_name_over_slug_lookup():
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo Cube"},
        },
        "2": {"class_type": "KSampler", "inputs": {"image": ["1", 0]}},
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "Demo Cube",
                "value": ["2", 0],
            },
        },
    }

    analysis = analyze_cubes(prompt, default_alias_lookup={cube_id: "demo_cube"})

    assert analysis.cubes[cube_id].name == "Demo Cube"


def test_analyze_cubes_preserves_underscore_live_name_over_space_lookup():
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "demo_cube"},
        },
        "2": {"class_type": "KSampler", "inputs": {"image": ["1", 0]}},
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id,
                "default_alias": "demo_cube",
                "value": ["2", 0],
            },
        },
    }

    analysis = analyze_cubes(prompt, default_alias_lookup={cube_id: "Demo Cube"})

    assert analysis.cubes[cube_id].name == "demo_cube"


def test_workflow_links_update_existing_nodes():
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "Any Switch (rgthree)",
            "inputs": {"any_02": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 2,
                "type": "Any Switch (rgthree)",
                "inputs": [
                    {"name": "any_01", "link": 5},
                    {"name": "any_02", "link": 6},
                ],
            },
            {"id": 10, "type": "VAELoader", "inputs": []},
        ],
        "links": [
            [5, 10, 0, 2, 0, "*"],
            [6, 1, 0, 2, 1, "*"],
        ],
        "definitions": {"subgraphs": []},
    }

    analysis = analyze_cubes(prompt, workflow=workflow)

    cube = analysis.cubes[cube_id]
    assert "10" in cube.subgraph_nodes


def test_export_filters_cube_ids():
    cube_id_a = "local/example-user/alpha.cube"
    cube_id_b = "local/example-user/beta.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id_a, "default_alias": "Alpha"},
        },
        "2": {"class_type": "KSampler", "inputs": {"image": ["1", 0]}},
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id_a,
                "default_alias": "Alpha",
                "value": ["2", 0],
            },
        },
        "4": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id_b, "default_alias": "Beta"},
        },
        "5": {"class_type": "VAELoader", "inputs": {"vae_name": ["4", 0]}},
        "6": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": cube_id_b,
                "default_alias": "Beta",
                "value": ["5", 0],
            },
        },
    }

    exported = export_cubes(
        prompt, cube_ids=[cube_id_a], definition_resolver=lambda _: {}
    )

    assert len(exported) == 1
    assert exported[0].default_alias == "Alpha"
