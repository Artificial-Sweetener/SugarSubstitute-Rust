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
"""Verify which mask files a workflow names and what a project calls each."""

from __future__ import annotations

from sugarcubes.authoring.workflow_sugarscript_projection import (
    ProjectMaskFile,
    project_mask_files,
)
from sugarcubes.workflow_analysis import CubeGraphAnalysisService


def _workflow() -> dict[str, object]:
    """Build a workflow of two Cubes that load masks, one of them by a list."""

    def document(cube_id: str, nodes: dict[str, object]) -> dict[str, object]:
        return {
            "cube_id": cube_id,
            "version": "1.0.0",
            "metadata": {"default_alias": "masks"},
            "implementation": {
                "nodes": nodes,
                "inputs": {},
                "outputs": {},
                "layout": {"origin": [0, 0], "ds": {}, "nodes": {}, "markers": {}},
                "definitions": {},
                "subgraphs": [],
            },
            "surface": {"default_flavor_id": "default", "controls": []},
            "flavors": {
                "authored": [{"id": "default", "name": "Default", "values": {}}]
            },
        }

    regions = document(
        "artificial-sweetener/base-cubes/regions.cube",
        {
            "load_mask_batch": {
                "class_type": "SimpleSyrup.LoadMaskBatch",
                "inputs": {
                    "channel": "red",
                    "image": [
                        "sugarcubes/sources/project-0a1b2c3d-left side.png",
                        "sugarcubes/sources/source-11111111.png",
                        "sugarcubes/sources/project-0a1b2c3d-left side.png",
                    ],
                },
            },
            "sampler": {
                "class_type": "KSampler",
                "inputs": {"image": "not a mask.png"},
            },
        },
    )
    inpaint = document(
        "artificial-sweetener/base-cubes/inpaint.cube",
        {
            "load_image_as_mask": {
                "class_type": "LoadImageMask",
                "inputs": {"image": "loose.jpg", "channel": "alpha"},
            }
        },
    )
    cubes = (("one", "Prompt: by/Region", regions), ("two", "Inpaint", inpaint))
    return {
        "version": 0.4,
        "links": [],
        "nodes": [
            {
                "id": index,
                "type": f"definition-{index}",
                "mode": 0,
                "pos": [index * 2000.0, 0.0],
                "size": [320.0, 200.0],
                "inputs": [],
                "outputs": [],
                "properties": {
                    "sugarcubes_kind": "cube",
                    "sugarcubes_cube": {
                        "cube_id": payload["cube_id"],
                        "cube_version": "1.0.0",
                        "default_alias": alias,
                        "instance_id": instance,
                        "instance_alias": alias,
                    },
                },
            }
            for index, (instance, alias, payload) in enumerate(cubes, start=1)
        ],
        "definitions": {
            "subgraphs": [
                {
                    "id": f"definition-{index}",
                    "name": alias,
                    "nodes": [],
                    "links": [],
                    "inputs": [],
                    "outputs": [],
                    "extra": {
                        "sugarcubes_kind": "cube",
                        "sugarcubes_cube": {
                            "cube_id": payload["cube_id"],
                            "cube_version": "1.0.0",
                            "default_alias": alias,
                        },
                        "sugarcubes_document": payload,
                    },
                }
                for index, (_, alias, payload) in enumerate(cubes, start=1)
            ]
        },
    }


def test_a_projects_own_mask_keeps_its_name_and_any_other_is_named_for_its_node() -> (
    None
):
    """Name each mask file once: by its project name, or by the Cube and node that load it."""

    files = project_mask_files(CubeGraphAnalysisService().analyze(_workflow()))

    assert files == (
        ProjectMaskFile(
            "sugarcubes/sources/project-0a1b2c3d-left side.png", "left side.png"
        ),
        ProjectMaskFile(
            "sugarcubes/sources/source-11111111.png",
            "Prompt_by_Region__load_mask_batch-2.png",
        ),
        ProjectMaskFile("loose.jpg", "Inpaint__load_image_as_mask.jpg"),
    )
