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
"""Build deterministic native workflows for canonical contract proofs."""

from __future__ import annotations

from copy import deepcopy


def cube_document() -> dict[str, object]:
    """Return one minimal canonical Cube document embedded with a workflow."""

    return {
        "cube_id": "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube",
        "version": "1.2.0",
        "description": "Create an SDXL image.",
        "metadata": {"default_alias": "SDXL/Text to Image"},
        "implementation": {
            "nodes": {
                "checkpoint": {
                    "class_type": "SimpleSyrup.SimpleLoadCheckpoint",
                    "inputs": {},
                }
            },
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }


def cube_workflow() -> dict[str, object]:
    """Return one self-contained wild Cube workflow with an ordinary loose node."""

    definition_identity = {
        "cube_id": "Artificial-Sweetener/Base-Cubes/SDXL/Text to Image.cube",
        "cube_version": "1.2.0",
        "default_alias": "SDXL/Text to Image",
        "provenance": {"repo_ref": "Artificial-Sweetener/Base-Cubes"},
    }
    return {
        "version": 0.4,
        "nodes": [
            {
                "id": 7,
                "type": "sdxl-text-definition",
                "properties": {
                    "sugarcubes_kind": "cube",
                    "sugarcubes_cube": {
                        **deepcopy(definition_identity),
                        "instance_id": "cube-sdxl-text-1",
                        "instance_alias": "Hero image",
                    },
                    "sugarcubes_surface": {},
                },
                "inputs": [],
                "outputs": [],
            },
            {"id": 9, "type": "PreviewImage", "properties": {}},
        ],
        "links": [],
        "definitions": {
            "subgraphs": [
                {
                    "id": "sdxl-text-definition",
                    "name": "SDXL/Text to Image",
                    "nodes": [{"id": 1, "type": "SimpleSyrup.SimpleLoadCheckpoint"}],
                    "links": [],
                    "inputs": [],
                    "outputs": [],
                    "extra": {
                        "sugarcubes_kind": "cube",
                        "sugarcubes_cube": deepcopy(definition_identity),
                        "sugarcubes_document": cube_document(),
                    },
                }
            ]
        },
    }
