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
"""Prove nested subgraph wrapper controls preserve their authored defaults."""

from __future__ import annotations

from copy import deepcopy
from typing import Any

from sugarcubes.cube_model.version_policy import suggest_version
from sugarcubes.exporter import export_cubes

WRAPPER_ID = "ff5e3223-5b47-497a-944e-4c26c8c55545"
CUBE_ID = "local/personal/Krea2/Text to Image.cube"


def test_krea_wrapper_values_become_portable_surface_defaults() -> None:
    """Persist the containing cube's wrapper values instead of nested fallbacks."""

    cube = export_cubes(
        _prompt(),
        workflow=_workflow(),
        definition_resolver=_definition,
    )[0].cube

    controls = {
        control["input_name"]: control
        for control in cube["surface"]["controls"]
        if control["symbol"] == "ksampler"
    }
    authored = cube["flavors"]["authored"][0]["values"]

    assert set(controls) == {
        "width",
        "height",
        "seed",
        "steps",
        "cfg",
        "sampler_name",
        "scheduler",
        "batch_size",
    }
    assert {
        name: authored[control["control_id"]]
        for name, control in controls.items()
        if name != "seed"
    } == {
        "width": 1216,
        "height": 1536,
        "steps": 8,
        "cfg": 1.0,
        "sampler_name": "euler",
        "scheduler": "normal",
        "batch_size": 1,
    }
    assert controls["seed"]["control_id"] not in authored
    assert "denoise" not in controls
    assert WRAPPER_ID in cube["implementation"]["definitions"]


def test_krea_wrapper_surface_repair_advances_the_minor_version() -> None:
    """Classify newly preserved executable defaults as an implementation repair."""

    repaired = export_cubes(
        _prompt(),
        workflow=_workflow(),
        definition_resolver=_definition,
    )[0].cube
    existing = deepcopy(repaired)
    existing["version"] = "1.2.0"
    repaired["version"] = "1.2.0"
    existing["implementation"]["definitions"].pop(WRAPPER_ID)
    existing["surface"]["controls"] = [
        control
        for control in existing["surface"]["controls"]
        if control["symbol"] != "ksampler"
    ]
    existing["flavors"]["authored"][0]["values"] = {
        control_id: value
        for control_id, value in existing["flavors"]["authored"][0]["values"].items()
        if not control_id.startswith("ksampler.")
    }

    suggestion = suggest_version(existing, repaired)

    assert suggestion.bump == "minor"
    assert suggestion.suggested == "1.3.0"
    assert suggestion.reason == "Implementation changed"


def test_connected_wrapper_input_remains_an_implementation_binding() -> None:
    """Keep a boundary-fed wrapper input out of the containing Cube surface."""

    prompt = _prompt()
    prompt["1"]["inputs"]["width"] = ["3", 0]
    prompt["3"] = {
        "class_type": "SugarCubes.CubeInput",
        "inputs": {
            "cube_id": CUBE_ID,
            "default_alias": "Krea2/Text to Image",
        },
    }

    cube = export_cubes(
        prompt,
        workflow=_workflow(),
        definition_resolver=_definition,
    )[0].cube

    wrapper_controls = {
        control["input_name"]
        for control in cube["surface"]["controls"]
        if control["symbol"] == "ksampler"
    }
    assert "width" not in wrapper_controls
    assert cube["implementation"]["nodes"]["ksampler"]["inputs"]["width"] == [
        "@binding",
        "input.int",
    ]
    assert cube["implementation"]["inputs"]["input.int"]["targets"] == [
        ["ksampler", "width"]
    ]


def _prompt() -> dict[str, Any]:
    """Return the flattened prompt shape emitted for the hostile Krea cube."""

    return {
        "1": {
            "class_type": WRAPPER_ID,
            "inputs": {},
            "_meta": {"title": "KSampler"},
        },
        "2": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": CUBE_ID,
                "default_alias": "Krea2/Text to Image",
                "value": ["1", 0],
            },
        },
    }


def _workflow() -> dict[str, Any]:
    """Return the nested wrapper and conflicting implementation defaults."""

    interface_names = [
        "width",
        "height",
        "seed",
        "steps",
        "cfg",
        "sampler_name",
        "scheduler",
        "batch_size",
    ]
    interface_types = ["INT", "INT", "INT", "INT", "FLOAT", "COMBO", "COMBO", "INT"]
    links = [
        _boundary_link(100, 0, 1887, 0, "INT"),
        _boundary_link(101, 1, 1887, 1, "INT"),
        _boundary_link(102, 2, 1886, 0, "INT"),
        _boundary_link(103, 3, 1886, 1, "INT"),
        _boundary_link(104, 4, 1886, 2, "FLOAT"),
        _boundary_link(105, 5, 1886, 3, "COMBO"),
        _boundary_link(106, 6, 1886, 4, "COMBO"),
        _boundary_link(107, 7, 1887, 2, "INT"),
    ]
    return {
        "nodes": [
            {
                "id": 1,
                "type": WRAPPER_ID,
                "title": "KSampler",
                "inputs": [
                    {
                        "name": name,
                        "type": input_type,
                        "link": None,
                        "widget": {"name": name},
                    }
                    for name, input_type in zip(interface_names, interface_types)
                ],
                "widgets_values": [1216, 1536, 0, 8, 1.0, "euler", "normal", 1],
                "sugarcubes_widget_values": {
                    "width": 1216,
                    "height": 1536,
                    "seed": 0,
                    "steps": 8,
                    "cfg": 1.0,
                    "sampler_name": "euler",
                    "scheduler": "normal",
                    "batch_size": 1,
                },
            },
            {"id": 2, "type": "SugarCubes.CubeOutput"},
        ],
        "definitions": {
            "subgraphs": [
                {
                    "id": WRAPPER_ID,
                    "name": "KSampler",
                    "inputs": [
                        {
                            "id": f"input-{index}",
                            "name": name,
                            "label": name,
                            "type": input_type,
                            "linkIds": [100 + index],
                        }
                        for index, (name, input_type) in enumerate(
                            zip(interface_names, interface_types)
                        )
                    ],
                    "outputs": [],
                    "nodes": [
                        {
                            "id": 1887,
                            "type": "EmptyLatentImage",
                            "inputs": [
                                _widget_input("width", 100),
                                _widget_input("height", 101),
                                _widget_input("batch_size", 107),
                            ],
                            "widgets_values": [1080, 1512, 1],
                        },
                        {
                            "id": 1886,
                            "type": "SimpleSyrup.KSamplerExtras",
                            "inputs": [
                                _widget_input("seed", 102),
                                _widget_input("steps", 103),
                                _widget_input("cfg", 104),
                                _widget_input("sampler_name", 105),
                                _widget_input("scheduler", 106),
                                _widget_input("denoise", None),
                            ],
                            "widgets_values": [
                                0,
                                "randomize",
                                30,
                                6.0,
                                "er_sde",
                                "simple",
                                1.0,
                            ],
                        },
                    ],
                    "links": links,
                }
            ]
        },
        "version": 1,
    }


def _boundary_link(
    link_id: int,
    origin_slot: int,
    target_id: int,
    target_slot: int,
    input_type: str,
) -> dict[str, Any]:
    """Build one serialized subgraph-boundary link."""

    return {
        "id": link_id,
        "origin_id": -10,
        "origin_slot": origin_slot,
        "target_id": target_id,
        "target_slot": target_slot,
        "type": input_type,
    }


def _widget_input(name: str, link: int | None) -> dict[str, Any]:
    """Build one name-addressed inner widget input."""

    return {
        "name": name,
        "type": "*",
        "link": link,
        "widget": {"name": name},
    }


def _definition(class_type: str) -> dict[str, Any]:
    """Return the exact inner definitions used by the Krea sampler subgraph."""

    if class_type == "EmptyLatentImage":
        required = {
            "width": ["INT", {"default": 512, "min": 16, "max": 16384, "step": 8}],
            "height": ["INT", {"default": 512, "min": 16, "max": 16384, "step": 8}],
            "batch_size": ["INT", {"default": 1, "min": 1, "max": 4096}],
        }
        return {
            "input": {"required": required},
            "input_order": {"required": list(required)},
            "output": ["LATENT"],
            "output_name": ["LATENT"],
            "output_is_list": [False],
        }
    if class_type == "SimpleSyrup.KSamplerExtras":
        required = {
            "seed": [
                "INT",
                {
                    "default": 0,
                    "min": 0,
                    "max": 2**64 - 1,
                    "control_after_generate": True,
                },
            ],
            "steps": ["INT", {"default": 20, "min": 1, "max": 10000}],
            "cfg": ["FLOAT", {"default": 8.0, "min": 0.0, "max": 100.0}],
            "sampler_name": [["euler", "er_sde"], {"default": "euler"}],
            "scheduler": [["normal", "simple"], {"default": "normal"}],
            "denoise": ["FLOAT", {"default": 1.0, "min": 0.0, "max": 1.0}],
        }
        return {
            "input": {"required": required},
            "input_order": {"required": list(required)},
            "output": ["LATENT"],
            "output_name": ["LATENT"],
            "output_is_list": [False],
        }
    return {}
