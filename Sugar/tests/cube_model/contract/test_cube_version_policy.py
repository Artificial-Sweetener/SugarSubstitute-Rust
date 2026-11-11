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
"""Semantic version policy coverage for canonical SugarCube documents."""

from __future__ import annotations

from copy import deepcopy
from typing import Any

from sugarcubes.cube_model.version_policy import suggest_version


def _build_document_payload() -> dict[str, Any]:
    return {
        "cube_id": "artificial-sweetener/base-cubes/text to image.cube",
        "version": "1.0.2",
        "description": "Text to image",
        "metadata": {"author": "Alice"},
        "implementation": {
            "nodes": {
                "ksampler": {
                    "class_type": "KSampler",
                    "inputs": {"latent": ["@binding", "input.latent"]},
                },
                "wrapper": {
                    "class_type": "94f725d5-39bf-4060-be68-f573214a2055",
                    "inputs": {},
                },
            },
            "inputs": {
                "input.latent": {
                    "kind": "input",
                    "targets": [["ksampler", "latent"]],
                }
            },
            "outputs": {"output.image": "ksampler"},
            "layout": {"origin": [0, 0], "nodes": {}, "groups": []},
            "definitions": {"KSampler": {}},
            "subgraphs": [
                {
                    "id": "94f725d5-39bf-4060-be68-f573214a2055",
                    "nodes": [
                        {
                            "id": 1,
                            "type": "CLIPTextEncode",
                            "inputs": {"text": "prompt"},
                        }
                    ],
                    "links": [],
                    "inputs": [],
                    "outputs": [],
                }
            ],
        },
        "surface": {
            "default_flavor_id": "default",
            "controls": [
                {
                    "control_id": "ksampler.cfg",
                    "symbol": "ksampler",
                    "input_name": "cfg",
                    "label": "cfg",
                    "class_type": "KSampler",
                    "value_type": "number",
                }
            ],
        },
        "flavors": {
            "authored": [
                {
                    "id": "default",
                    "name": "Default",
                    "values": {"ksampler.cfg": 7},
                },
            ]
        },
    }


def test_version_policy_reports_patch_for_authored_flavor_change() -> None:
    old_cube = _build_document_payload()
    new_cube = deepcopy(old_cube)
    new_cube["flavors"]["authored"][0]["values"]["ksampler.cfg"] = 8

    suggestion = suggest_version(old_cube, new_cube)

    assert suggestion.bump == "patch"
    assert suggestion.suggested == "1.0.3"
    assert suggestion.reason == "Authored flavor changed"


def test_version_policy_reports_minor_for_implementation_change() -> None:
    old_cube = _build_document_payload()
    new_cube = deepcopy(old_cube)
    new_cube["implementation"]["nodes"]["ksampler"]["class_type"] = "KSamplerAdvanced"

    suggestion = suggest_version(old_cube, new_cube)

    assert suggestion.bump == "minor"
    assert suggestion.suggested == "1.1.0"
    assert suggestion.reason == "Implementation changed"


def test_version_policy_reports_major_for_interface_change() -> None:
    old_cube = _build_document_payload()
    new_cube = deepcopy(old_cube)
    new_cube["implementation"]["outputs"]["output.mask"] = "ksampler"

    suggestion = suggest_version(old_cube, new_cube)

    assert suggestion.bump == "major"
    assert suggestion.suggested == "2.0.0"
    assert suggestion.reason == "Interface changed"


def test_version_policy_ignores_cosmetic_only_changes() -> None:
    old_cube = _build_document_payload()
    new_cube = deepcopy(old_cube)
    new_cube["implementation"]["layout"]["origin"] = [100, 200]

    suggestion = suggest_version(old_cube, new_cube)

    assert suggestion.bump == "none"
    assert suggestion.suggested == "1.0.2"
    assert suggestion.reason == "Cosmetic only"
