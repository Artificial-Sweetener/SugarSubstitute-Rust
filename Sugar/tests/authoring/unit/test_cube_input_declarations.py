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
"""Verify what a Cube document declares about its inputs: names, decimals, and its face."""

from __future__ import annotations

from sugarcubes.authoring.cube_input_declarations import CubeInputDeclarations
from sugarcubes.cube_model import CubeDocument


def _document() -> CubeDocument:
    """Build a Cube with a plain node, a wrapper node, and one boundary-fed input."""

    return CubeDocument.from_dict(
        {
            "cube_id": "artificial-sweetener/base-cubes/declared.cube",
            "version": "1.0.0",
            "metadata": {"default_alias": "declared"},
            "implementation": {
                "nodes": {
                    "sampler": {
                        "class_type": "KSampler",
                        "inputs": {"image": None, "cfg": 4, "steps": 10, "seed": 1},
                    },
                    "upscale": {
                        "class_type": "wrapper-1",
                        "inputs": {"factor": 2, "passes": 1},
                    },
                },
                "inputs": {
                    "input.image": {
                        "kind": "input",
                        "targets": [["sampler", "image"]],
                    }
                },
                "outputs": {"output.image": ["sampler", 0]},
                "layout": {"origin": [0, 0], "ds": {}, "nodes": {}, "markers": {}},
                "definitions": {
                    "KSampler": {
                        "input": {
                            "required": {
                                "cfg": ["FLOAT", {"default": 7.0}],
                                "steps": ["INT", {"label": "Step count"}],
                            },
                            "optional": {
                                "seed": ["INT", {"localized_name": "Seed number"}]
                            },
                        }
                    }
                },
                "subgraphs": [
                    {
                        "id": "wrapper-1",
                        "name": "Upscale",
                        "inputs": [
                            {"name": "factor", "type": "FLOAT", "label": "scale by"},
                            {"name": "passes", "type": "INT", "label": "pass count"},
                        ],
                        "nodes": [],
                        "links": [],
                    }
                ],
            },
            "surface": {
                "default_flavor_id": "default",
                "controls": [
                    {
                        "control_id": "sampler.cfg",
                        "symbol": "sampler",
                        "input_name": "cfg",
                        "label": "Guidance",
                        "class_type": "KSampler",
                        "value_type": "number",
                    },
                    {
                        "control_id": "sampler.image",
                        "symbol": "sampler",
                        "input_name": "image",
                        "label": "Image",
                        "class_type": "KSampler",
                        "value_type": "string",
                    },
                ],
            },
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }
    )


def test_an_input_is_named_by_the_face_then_its_definition_then_its_wrapper() -> None:
    """Name each input as a person sees it, falling back to its key."""

    document = _document()
    declarations = CubeInputDeclarations(document)
    sampler = document.implementation.nodes["sampler"]
    upscale = document.implementation.nodes["upscale"]

    assert declarations.label("sampler", sampler, "cfg") == "Guidance"
    assert declarations.label("sampler", sampler, "steps") == "Step count"
    assert declarations.label("sampler", sampler, "seed") == "Seed number"
    assert declarations.label("upscale", upscale, "factor") == "scale by"
    assert declarations.label("upscale", upscale, "passes") == "pass count"
    assert declarations.label("sampler", sampler, "undeclared") == "undeclared"


def test_a_whole_number_held_for_a_decimal_input_is_a_decimal() -> None:
    """Restore the decimal a page returns as a whole number, and nothing else."""

    document = _document()
    declarations = CubeInputDeclarations(document)
    sampler = document.implementation.nodes["sampler"]
    upscale = document.implementation.nodes["upscale"]

    cfg = declarations.typed(sampler, "cfg", 4)
    assert cfg == 4.0 and isinstance(cfg, float)
    factor = declarations.typed(upscale, "factor", 2)
    assert factor == 2.0 and isinstance(factor, float)
    assert isinstance(declarations.typed(sampler, "steps", 10), int)
    assert isinstance(declarations.typed(upscale, "passes", 1), int)
    assert declarations.typed(sampler, "cfg", True) is True
    assert declarations.typed(sampler, "cfg", "4") == "4"
    assert isinstance(declarations.typed(sampler, "undeclared", 3), int)


def test_the_face_is_what_a_person_edits_and_never_what_another_cube_supplies() -> None:
    """Expose surface controls, without a field the Cube's boundary feeds."""

    assert CubeInputDeclarations(_document()).exposed == frozenset({("sampler", "cfg")})
