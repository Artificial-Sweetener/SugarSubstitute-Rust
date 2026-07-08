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
import json

import pytest

from sugarcubes.importer import CubeImportError, load_cube, prepare_import
from sugarcubes.importer import loader as loader_module


def _build_current_payload() -> dict:
    return {
        "description": "demo",
        "cube_id": "artificial-sweetener/base-cubes/demo.cube",
        "version": "1.0.0",
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }


PARENT_SUBGRAPH_ID = "644694cf-354b-4cc8-8a67-a78145a8180e"
CHILD_SUBGRAPH_ID = "8f6c43da-07af-4666-9e9a-0b4c7f83bdad"


def test_load_cube_warns_on_invalid_input_targets(tmp_path):
    payload = _build_current_payload()
    payload["implementation"]["inputs"] = {
        "input.value": {
            "kind": "input",
            "targets": ["bad", ["node", 0], ["node", 1, 2]],
        }
    }
    path = tmp_path / "demo.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(path)
    assert any("target #1 is invalid" in warning for warning in loaded.warnings)


def test_load_cube_rejects_inherit_input_kind(tmp_path):
    payload = _build_current_payload()
    payload["implementation"]["inputs"] = {
        "input.model": {
            "kind": "inherit",
            "targets": [],
        }
    }
    path = tmp_path / "inherit.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CubeImportError, match="unsupported kind 'inherit'"):
        load_cube(path)


def test_prepare_import_includes_route_identity_metadata(tmp_path):
    payload = _build_current_payload()
    payload["cube_id"] = "artificial-sweetener/base-cubes/SDXL/Text to Image.cube"
    payload["metadata"] = {
        "default_alias": "SDXL/Text to Image",
        "target_model": "SDXL",
    }
    payload["implementation"]["inputs"] = {
        "input.value": {"kind": "input", "targets": []}
    }
    path = tmp_path / "Text to Image.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    prepared = prepare_import(load_cube(path))

    assert prepared.cube["default_alias"] == "SDXL/Text to Image"
    assert prepared.cube["target_model"] == "SDXL"
    assert prepared.markers[0]["widget_values"]["default_alias"] == "SDXL/Text to Image"
    assert (
        prepared.markers[0]["widget_values"]["instance_alias"] == "SDXL/Text to Image"
    )


def test_load_cube_rejects_default_alias_that_conflicts_with_route(tmp_path):
    payload = _build_current_payload()
    payload["cube_id"] = "artificial-sweetener/base-cubes/SDXL/Text to Image.cube"
    payload["metadata"] = {"default_alias": "Text to Image"}
    path = tmp_path / "Text to Image.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CubeImportError, match="default_alias must match cube route"):
        load_cube(path)


def test_load_cube_layout_warnings(tmp_path):
    payload = _build_current_payload()
    payload["implementation"]["nodes"] = {
        "node": {"class_type": "KSampler", "inputs": {}}
    }
    payload["implementation"]["definitions"] = {"KSampler": {}}
    payload["implementation"]["layout"] = {"origin": [0, 0], "nodes": {}}
    path = tmp_path / "layout.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(path)
    assert any("Layout missing node entry" in warning for warning in loaded.warnings)


def test_prepare_import_preserves_valid_node_execution_mode(tmp_path):
    payload = _build_current_payload()
    payload["implementation"]["nodes"] = {
        "vae_override": {"class_type": "VAELoader", "inputs": {}, "mode": 4}
    }
    payload["implementation"]["definitions"] = {"VAELoader": {}}
    payload["implementation"]["layout"] = {
        "origin": [0, 0],
        "nodes": {
            "vae_override": {
                "id": "10",
                "class_type": "VAELoader",
                "pos": [100, 100],
                "size": [180, 60],
            }
        },
    }
    path = tmp_path / "mode.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(path)
    prepared = prepare_import(loaded)

    assert loaded.nodes["vae_override"].data["mode"] == 4
    assert prepared.nodes[0]["mode"] == 4


def test_load_cube_ignores_invalid_node_execution_mode(tmp_path):
    payload = _build_current_payload()
    payload["implementation"]["nodes"] = {
        "vae_override": {"class_type": "VAELoader", "inputs": {}, "mode": "4"}
    }
    payload["implementation"]["definitions"] = {"VAELoader": {}}
    payload["implementation"]["layout"] = {
        "origin": [0, 0],
        "nodes": {
            "vae_override": {
                "id": "10",
                "class_type": "VAELoader",
                "pos": [100, 100],
                "size": [180, 60],
            }
        },
    }
    path = tmp_path / "invalid_mode.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(path)
    prepared = prepare_import(loaded)

    assert "mode" not in loaded.nodes["vae_override"].data
    assert "mode" not in prepared.nodes[0]
    assert any(
        "mode is invalid and was ignored" in warning for warning in loaded.warnings
    )


def test_load_cube_does_not_promote_group_title_to_default_alias(tmp_path):
    payload = _build_current_payload()
    payload["metadata"] = {"cube_name": "Ignored Legacy Name"}
    payload["implementation"]["inputs"] = {
        "input.value": {"kind": "input", "targets": []}
    }
    payload["implementation"]["layout"] = {
        "origin": [0, 0],
        "nodes": {},
        "markers": {},
        "groups": [
            {
                "title": "Polluted Instance Alias",
                "sugarcubes": {
                    "cube_id": payload["cube_id"],
                    "cube_name": "Ignored Legacy Group Name",
                    "instance_alias": "Polluted Instance Alias",
                },
            }
        ],
    }
    path = tmp_path / "file_stem_name.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(path)

    assert loaded.markers["input.value"].widget_values["default_alias"] == "demo"
    assert loaded.markers["input.value"].widget_values["instance_alias"] == "demo"


def test_load_cube_rejects_bad_outputs(tmp_path):
    payload = _build_current_payload()
    payload["implementation"]["outputs"] = {"output.value": []}
    path = tmp_path / "outputs.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CubeImportError, match="must specify a node reference"):
        load_cube(path)


def test_load_cube_uses_unique_group_default_alias_when_metadata_is_absent(tmp_path):
    payload = _build_current_payload()
    payload["metadata"] = {}
    payload["implementation"]["inputs"] = {
        "input.value": {"kind": "input", "targets": []}
    }
    payload["implementation"]["layout"] = {
        "origin": [0, 0],
        "nodes": {},
        "markers": {},
        "groups": [
            {
                "title": "Polluted Instance Alias",
                "sugarcubes": {
                    "cube_id": payload["cube_id"],
                    "default_alias": "Text to Image",
                    "instance_alias": "Polluted Instance Alias",
                },
            }
        ],
    }
    path = tmp_path / "text to image.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(path)

    assert loaded.markers["input.value"].widget_values["default_alias"] == "demo"


def test_load_cube_rejects_legacy_runtime_payload(tmp_path):
    payload = {
        "cube_id": "artificial-sweetener/base-cubes/demo.cube",
        "version": "1.0.0",
        "nodes": {},
    }
    path = tmp_path / "legacy.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(
        CubeImportError,
        match="Legacy cube format is unsupported. Run scripts/migrate_legacy_cubes.py.",
    ) as exc_info:
        load_cube(path)

    assert exc_info.value.details == {
        "legacy": True,
        "cube_id": "artificial-sweetener/base-cubes/demo.cube",
        "path": str(path),
    }


def test_loader_handles_missing_optional_comfy_nodes_runtime(monkeypatch):
    monkeypatch.setattr(loader_module, "_load_comfy_nodes_module", lambda: None)

    assert loader_module._has_definition("NotInstalled", {}) is False


def test_load_cube_treats_uuid_wrapper_nodes_as_defined_subgraphs(tmp_path):
    payload = _build_current_payload()
    subgraph_id = "bc2b2877-06d7-4b9f-881a-9263df411f13"
    payload["implementation"]["nodes"] = {
        "wrapper": {
            "class_type": subgraph_id,
            "inputs": {"negative_prompt": ["prompt", 0]},
        }
    }
    payload["implementation"]["subgraphs"] = [
        {
            "id": subgraph_id,
            "nodes": [{"id": 1, "type": "String", "inputs": [], "outputs": []}],
            "links": [],
            "groups": [],
            "config": {},
            "extra": {},
            "inputs": [],
            "outputs": [],
        }
    ]
    path = tmp_path / "subgraph_wrapper.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(path)

    assert not any(
        "Undefined node classes referenced" in warning for warning in loaded.warnings
    )


def test_load_cube_accepts_complete_nested_subgraph_definitions(tmp_path):
    payload = _build_current_payload()
    payload["implementation"]["nodes"] = {
        "wrapper": {"class_type": PARENT_SUBGRAPH_ID, "inputs": {}}
    }
    payload["implementation"]["subgraphs"] = [
        {
            "id": CHILD_SUBGRAPH_ID,
            "name": "Scale Masked Area by Factor",
            "nodes": [{"id": 2, "type": "KSampler"}],
            "links": [],
            "inputs": [],
            "outputs": [],
        },
        {
            "id": PARENT_SUBGRAPH_ID,
            "name": "Detailer",
            "nodes": [{"id": 1, "type": CHILD_SUBGRAPH_ID}],
            "links": [],
            "inputs": [],
            "outputs": [],
        },
    ]
    path = tmp_path / "nested_subgraphs.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(path)
    prepared = prepare_import(loaded)

    assert [entry["id"] for entry in prepared.subgraphs] == [
        CHILD_SUBGRAPH_ID,
        PARENT_SUBGRAPH_ID,
    ]


def test_prepare_import_preserves_text_to_image_prompt_nodes(tmp_path):
    payload = _build_current_payload()
    payload["cube_id"] = "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube"
    payload["version"] = "1.1.1"
    payload["implementation"]["nodes"] = {
        "positive_prompt": {
            "class_type": "PrimitiveStringMultiline",
            "inputs": {},
        },
        "negative_prompt": {
            "class_type": "PrimitiveStringMultiline",
            "inputs": {},
        },
        "prompt_encode_style": {
            "class_type": "SimpleSyrup.PromptEncodeStyle",
            "inputs": {
                "positive": ["positive_prompt", 0],
                "negative": ["negative_prompt", 0],
            },
        },
    }
    payload["implementation"]["definitions"] = {
        "PrimitiveStringMultiline": {},
        "SimpleSyrup.PromptEncodeStyle": {},
    }

    cube_path = tmp_path / "text to image.cube"
    cube_path.write_text(json.dumps(payload), encoding="utf-8")

    loaded = load_cube(cube_path)
    prepared = prepare_import(loaded, drop_origin=(0.0, 0.0))

    prepared_symbols = {entry["symbol"] for entry in prepared.nodes}
    assert "positive_prompt" in prepared_symbols
    assert "negative_prompt" in prepared_symbols
    assert "prompt_encode_style" in prepared_symbols
