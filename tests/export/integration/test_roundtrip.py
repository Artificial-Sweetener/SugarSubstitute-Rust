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

from __future__ import annotations

from typing import Any

from pathlib import Path
import pytest

from sugarcubes.exporter import ExportedCube, export_cubes, write_cubes
from sugarcubes.importer import load_cube, prepare_import


def _build_prompt() -> Any:
    cube_id = "artificial-sweetener/base-cubes/demo.cube"
    return {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0], "seed": 1},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
    }


def _definition_resolver(_class_type: Any) -> Any:
    return {}


PARENT_SUBGRAPH_ID = "644694cf-354b-4cc8-8a67-a78145a8180e"
CHILD_SUBGRAPH_ID = "8f6c43da-07af-4666-9e9a-0b4c7f83bdad"
CYCLIC_SUBGRAPH_ID = "53f09d1e-0364-4cb3-b5e7-535f63d1323f"


def _build_subgraph_wrapper_prompt() -> Any:
    cube_id = "artificial-sweetener/base-cubes/demo.cube"
    return {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["10", 0]},
        },
    }


def _build_nested_subgraph_workflow(
    *,
    parent_id: Any = PARENT_SUBGRAPH_ID,
    child_id: Any = CHILD_SUBGRAPH_ID,
    include_child: Any = True,
) -> Any:
    subgraphs = [
        {
            "id": parent_id,
            "name": "Detailer",
            "nodes": [{"id": 20, "type": child_id}],
            "links": [],
            "inputs": [],
            "outputs": [],
        }
    ]
    if include_child:
        subgraphs.append(
            {
                "id": child_id,
                "name": "Scale Masked Area by Factor",
                "nodes": [{"id": 30, "type": "KSampler"}],
                "links": [],
                "inputs": [],
                "outputs": [],
            }
        )
    return {
        "nodes": [
            {
                "id": 10,
                "type": parent_id,
                "inputs": [{"name": "value", "link": 1}],
            },
        ],
        "links": [[1, 1, 0, 10, 0, "ANY"]],
        "definitions": {"subgraphs": subgraphs},
    }


def test_exporter_builds_cube_payload() -> None:
    cubes = export_cubes(_build_prompt(), definition_resolver=_definition_resolver)
    assert len(cubes) == 1

    payload = cubes[0].cube
    assert payload["cube_id"] == "artificial-sweetener/base-cubes/demo.cube"
    assert payload["version"] == "1.0.0"
    assert payload["description"] == "Auto-converted cube for demo"
    assert payload["metadata"]["default_alias"] == "demo"
    assert "KSampler" in payload["implementation"]["definitions"]
    assert len(payload["implementation"]["nodes"]) == 1
    assert payload["surface"]["default_flavor_id"] == "default"
    assert payload["flavors"]["authored"][0]["id"] == "default"

    node_payload = next(iter(payload["implementation"]["nodes"].values()))
    assert node_payload["class_type"] == "KSampler"


def test_exporter_includes_subgraph_definitions() -> None:
    subgraph_id = "94f725d5-39bf-4060-be68-f573214a2055"
    subgraph_node_id = "subnode-1"
    cube_id = "artificial-sweetener/base-cubes/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["10", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 10,
                "type": subgraph_id,
                "inputs": [{"name": "value", "link": 1}],
            },
        ],
        "links": [[1, 1, 0, 10, 0, "ANY"]],
        "definitions": {
            "subgraphs": [
                {
                    "id": subgraph_id,
                    "name": "Schedule & Encode Prompts",
                    "nodes": [{"id": subgraph_node_id, "type": "KSampler", "mode": 4}],
                    "links": [],
                    "inputs": [],
                    "outputs": [],
                    "groups": [{"title": "Editor Group", "bounding": [0, 0, 200, 100]}],
                    "state": {},
                    "version": 1,
                }
            ]
        },
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    payload = cubes[0].cube

    assert payload["implementation"]["subgraphs"][0]["id"] == subgraph_id
    assert "groups" not in payload["implementation"]["subgraphs"][0]
    assert "mode" not in payload["implementation"]["subgraphs"][0]["nodes"][0]


def test_exporter_rejects_missing_wrapper_subgraph_definition() -> None:
    subgraph_id = "94f725d5-39bf-4060-be68-f573214a2055"
    cube_id = "artificial-sweetener/base-cubes/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["10", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 10,
                "type": subgraph_id,
                "inputs": [{"name": "value", "link": 1}],
            },
        ],
        "links": [[1, 1, 0, 10, 0, "ANY"]],
        "definitions": {"subgraphs": []},
    }

    with pytest.raises(ValueError, match="missing definition\\(s\\)"):
        export_cubes(
            prompt, workflow=workflow, definition_resolver=_definition_resolver
        )


def test_exporter_rejects_subgraph_without_executable_nodes() -> None:
    subgraph_id = "94f725d5-39bf-4060-be68-f573214a2055"
    cube_id = "artificial-sweetener/base-cubes/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["10", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 10,
                "type": subgraph_id,
                "inputs": [{"name": "value", "link": 1}],
            },
        ],
        "links": [[1, 1, 0, 10, 0, "ANY"]],
        "definitions": {"subgraphs": [{"id": subgraph_id, "nodes": []}]},
    }

    with pytest.raises(ValueError, match="must include executable nodes"):
        export_cubes(
            prompt, workflow=workflow, definition_resolver=_definition_resolver
        )


def test_exporter_includes_nested_subgraph_definitions_in_dependency_order() -> None:
    cubes = export_cubes(
        _build_subgraph_wrapper_prompt(),
        workflow=_build_nested_subgraph_workflow(),
        definition_resolver=_definition_resolver,
    )
    payload = cubes[0].cube

    subgraphs = payload["implementation"]["subgraphs"]
    subgraph_ids = [entry["id"] for entry in subgraphs]

    assert subgraph_ids == [CHILD_SUBGRAPH_ID, PARENT_SUBGRAPH_ID]
    assert subgraphs[0]["nodes"][0]["type"] == "KSampler"
    assert subgraphs[1]["nodes"][0]["type"] == CHILD_SUBGRAPH_ID


def test_exporter_rejects_missing_nested_subgraph_definition() -> None:
    with pytest.raises(ValueError, match=CHILD_SUBGRAPH_ID):
        export_cubes(
            _build_subgraph_wrapper_prompt(),
            workflow=_build_nested_subgraph_workflow(include_child=False),
            definition_resolver=_definition_resolver,
        )


def test_exporter_rejects_cyclic_nested_subgraph_definitions() -> None:
    workflow = _build_nested_subgraph_workflow(
        parent_id=PARENT_SUBGRAPH_ID,
        child_id=CHILD_SUBGRAPH_ID,
        include_child=False,
    )
    workflow["definitions"]["subgraphs"].append(
        {
            "id": CHILD_SUBGRAPH_ID,
            "name": "Nested Child",
            "nodes": [{"id": 30, "type": CYCLIC_SUBGRAPH_ID}],
            "links": [],
            "inputs": [],
            "outputs": [],
        }
    )
    workflow["definitions"]["subgraphs"].append(
        {
            "id": CYCLIC_SUBGRAPH_ID,
            "name": "Nested Cycle",
            "nodes": [{"id": 40, "type": PARENT_SUBGRAPH_ID}],
            "links": [],
            "inputs": [],
            "outputs": [],
        }
    )

    with pytest.raises(ValueError, match="cyclic nested subgraph references"):
        export_cubes(
            _build_subgraph_wrapper_prompt(),
            workflow=workflow,
            definition_resolver=_definition_resolver,
        )


def test_exporter_collects_real_node_definitions_inside_nested_subgraphs() -> None:
    cubes = export_cubes(
        _build_subgraph_wrapper_prompt(),
        workflow=_build_nested_subgraph_workflow(),
        definition_resolver=_definition_resolver,
    )
    definitions = cubes[0].cube["implementation"]["definitions"]

    assert "KSampler" in definitions
    assert CHILD_SUBGRAPH_ID not in definitions
    assert PARENT_SUBGRAPH_ID not in definitions


def test_export_import_roundtrip(tmp_path: Path) -> None:
    cubes = export_cubes(_build_prompt(), definition_resolver=_definition_resolver)
    saved = write_cubes(cubes, tmp_path, overwrite=True)
    cube_path = saved[0]["path"]

    loaded = load_cube(cube_path)
    prepared = prepare_import(loaded, drop_origin=(10, 20))

    assert prepared.nodes
    assert prepared.markers
    assert prepared.connections
    assert prepared.layout is not None
    assert prepared.layout["origin"] == [10, 20]
    assert any(
        "Layout origin missing or invalid" in warning for warning in prepared.warnings
    )


def test_write_cubes_uses_exact_filename_from_canonical_cube_id(tmp_path: Path) -> None:
    exported = ExportedCube(
        default_alias="Text to Image",
        cube={"cube_id": "local/personal/Text to Image.cube"},
        warnings=[],
    )

    saved = write_cubes([exported], tmp_path, overwrite=True)

    assert saved[0]["filename"] == "Text to Image.cube"
    assert (tmp_path / "Text to Image.cube").exists()


def test_export_import_roundtrip_preserves_subgraph_bodies(tmp_path: Path) -> None:
    subgraph_id = "94f725d5-39bf-4060-be68-f573214a2055"
    cube_id = "artificial-sweetener/base-cubes/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo"},
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["10", 0]},
        },
    }
    workflow = {
        "nodes": [
            {
                "id": 10,
                "type": subgraph_id,
                "inputs": [{"name": "value", "link": 1}],
            },
        ],
        "links": [[1, 1, 0, 10, 0, "ANY"]],
        "definitions": {
            "subgraphs": [
                {
                    "id": subgraph_id,
                    "nodes": [{"id": "1", "type": "KSampler"}],
                    "links": [],
                    "inputs": [],
                    "outputs": [],
                }
            ]
        },
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    saved = write_cubes(cubes, tmp_path, overwrite=True)
    loaded = load_cube(saved[0]["path"])
    prepared = prepare_import(loaded, drop_origin=(0, 0))

    assert prepared.subgraphs
    assert prepared.subgraphs[0]["id"] == subgraph_id
    assert prepared.subgraphs[0]["nodes"][0]["type"] == "KSampler"


def test_export_import_roundtrip_preserves_nested_subgraph_bodies(
    tmp_path: Path,
) -> None:
    cubes = export_cubes(
        _build_subgraph_wrapper_prompt(),
        workflow=_build_nested_subgraph_workflow(),
        definition_resolver=_definition_resolver,
    )
    saved = write_cubes(cubes, tmp_path, overwrite=True)
    loaded = load_cube(saved[0]["path"])
    prepared = prepare_import(loaded, drop_origin=(0, 0))

    subgraph_ids = [entry["id"] for entry in prepared.subgraphs]

    assert subgraph_ids == [CHILD_SUBGRAPH_ID, PARENT_SUBGRAPH_ID]
    assert prepared.subgraphs[1]["nodes"][0]["type"] == CHILD_SUBGRAPH_ID
    assert not any("subgraph" in warning.lower() for warning in prepared.warnings)


def test_export_preserves_group_metadata() -> None:
    prompt = _build_prompt()
    workflow = {
        "nodes": [
            {"id": 1, "type": "SugarCubes.CubeInput", "pos": [0, 0], "size": [140, 46]},
            {"id": 2, "type": "KSampler", "pos": [200, 0], "size": [180, 60]},
            {
                "id": 3,
                "type": "SugarCubes.CubeOutput",
                "pos": [420, 0],
                "size": [140, 46],
            },
        ],
        "groups": [
            {
                "title": "Custom Group",
                "bounding": [0, 0, 640, 200],
                "color": "#123456",
                "bgcolor": "#234567",
                "sugarcubes": {
                    "schema": 2,
                    "instance_id": "inst-1",
                    "cube_id": "artificial-sweetener/base-cubes/demo.cube",
                    "default_alias": "Demo",
                },
            }
        ],
        "version": 0,
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    payload = cubes[0].cube
    groups = payload["implementation"]["layout"]["groups"]
    assert groups[0]["title"] == "Custom Group"
    assert groups[0]["bounding"] == [-10.0, -60.0, 580.0, 130.0]
    assert groups[0]["sugarcubes"]["bounds"] == {
        "x": -10.0,
        "y": -60.0,
        "w": 580.0,
        "h": 130.0,
        "padding": {"x": 2.0, "y": 2.0, "top_extra": 0.0},
        "header": {"height": 32.0},
    }
    assert groups[0]["sugarcubes"]["default_alias"] == "Demo"
    assert "alias" not in groups[0]["sugarcubes"]


def test_export_allows_alias_divergence() -> None:
    prompt = _build_prompt()
    workflow = {
        "nodes": [
            {"id": 1, "type": "SugarCubes.CubeInput", "pos": [0, 0], "size": [140, 46]},
            {"id": 2, "type": "KSampler", "pos": [200, 0], "size": [180, 60]},
            {
                "id": 3,
                "type": "SugarCubes.CubeOutput",
                "pos": [420, 0],
                "size": [140, 46],
            },
        ],
        "groups": [
            {
                "title": "Demo Alias",
                "bounding": [0, 0, 640, 200],
                "color": "#123456",
                "bgcolor": "#234567",
                "sugarcubes": {
                    "schema": 2,
                    "instance_id": "inst-1",
                    "cube_id": "artificial-sweetener/base-cubes/demo.cube",
                    "default_alias": "Demo",
                    "alias": "Demo Alias",
                },
            }
        ],
        "version": 0,
    }

    cubes = export_cubes(
        prompt, workflow=workflow, definition_resolver=_definition_resolver
    )
    payload = cubes[0].cube
    groups = payload["implementation"]["layout"]["groups"]
    assert groups[0]["title"] == "Demo Alias"
    assert groups[0]["sugarcubes"]["default_alias"] == "Demo"
    assert "alias" not in groups[0]["sugarcubes"]
