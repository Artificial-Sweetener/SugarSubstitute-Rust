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
import pytest

from sugarcubes.exporter.graph import analyze_cubes
from sugarcubes.exporter.validation import CubeValidationError, validate


def test_marker_boundary_links_are_allowed() -> None:
    cube_id = "local/example-user/demo.cube"
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["10", 0]},
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {"cube_id": cube_id, "default_alias": "Demo", "value": ["2", 0]},
        },
        "10": {"class_type": "CheckpointLoaderSimple", "inputs": {}},
        "11": {"class_type": "PreviewImage", "inputs": {"images": ["3", 0]}},
    }

    analysis = analyze_cubes(prompt)
    validate(analysis)


def test_direct_cross_cube_links_fail() -> None:
    prompt = {
        "1": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {
                "cube_id": "local/example-user/alpha.cube",
                "default_alias": "Alpha",
            },
        },
        "2": {
            "class_type": "KSampler",
            "inputs": {"image": ["1", 0]},
        },
        "3": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": "local/example-user/alpha.cube",
                "default_alias": "Alpha",
                "value": ["2", 0],
            },
        },
        "4": {
            "class_type": "SugarCubes.CubeInput",
            "inputs": {
                "cube_id": "local/example-user/beta.cube",
                "default_alias": "Beta",
            },
        },
        "5": {
            "class_type": "VAELoader",
            "inputs": {"vae_name": ["4", 0], "samples": ["2", 0]},
        },
        "6": {
            "class_type": "SugarCubes.CubeOutput",
            "inputs": {
                "cube_id": "local/example-user/beta.cube",
                "default_alias": "Beta",
                "value": ["5", 0],
            },
        },
        "7": {"class_type": "PreviewImage", "inputs": {"images": ["5", 0]}},
    }

    analysis = analyze_cubes(prompt)
    with pytest.raises(CubeValidationError, match="Node belongs to multiple cubes"):
        validate(analysis)
