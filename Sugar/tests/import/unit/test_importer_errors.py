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

from pathlib import Path
import json
from typing import Any

import pytest

from sugarcubes.importer import CubeImportError, load_cube


def _base_current_payload() -> dict[str, Any]:
    return {
        "description": "demo",
        "cube_id": "artificial-sweetener/base-cubes/demo.cube",
        "version": "1.0.0",
        "metadata": {},
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


def test_load_cube_rejects_invalid_json(tmp_path: Path) -> None:
    path = tmp_path / "bad.cube"
    path.write_text("{not json}", encoding="utf-8")

    with pytest.raises(CubeImportError):
        load_cube(path)


def test_load_cube_rejects_invalid_nodes_shape(tmp_path: Path) -> None:
    payload = _base_current_payload()
    payload["implementation"]["nodes"] = []
    path = tmp_path / "bad_nodes.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CubeImportError, match="Expected an object-valued field"):
        load_cube(path)


def test_load_cube_requires_cube_id(tmp_path: Path) -> None:
    payload = _base_current_payload()
    del payload["cube_id"]
    path = tmp_path / "missing_id.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CubeImportError, match="Cube field 'cube_id' is required"):
        load_cube(path)


def test_load_cube_rejects_non_array_subgraphs(tmp_path: Path) -> None:
    payload = _base_current_payload()
    payload["implementation"]["subgraphs"] = {}
    path = tmp_path / "bad_subgraphs.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CubeImportError, match="Expected an array-valued field"):
        load_cube(path)


def test_load_cube_rejects_subgraph_without_nodes(tmp_path: Path) -> None:
    payload = _base_current_payload()
    payload["implementation"]["subgraphs"] = [
        {"id": "94f725d5-39bf-4060-be68-f573214a2055", "nodes": []}
    ]
    path = tmp_path / "bad_subgraph_nodes.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CubeImportError, match="must include a non-empty nodes array"):
        load_cube(path)


def test_load_cube_rejects_missing_nested_subgraph_definition(tmp_path: Path) -> None:
    payload = _base_current_payload()
    payload["implementation"]["nodes"] = {
        "wrapper": {"class_type": PARENT_SUBGRAPH_ID, "inputs": {}}
    }
    payload["implementation"]["subgraphs"] = [
        {
            "id": PARENT_SUBGRAPH_ID,
            "nodes": [{"id": 1, "type": CHILD_SUBGRAPH_ID}],
            "links": [],
            "inputs": [],
            "outputs": [],
        }
    ]
    path = tmp_path / "missing_nested_subgraph.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")

    with pytest.raises(CubeImportError, match=PARENT_SUBGRAPH_ID) as exc_info:
        load_cube(path)

    assert CHILD_SUBGRAPH_ID in str(exc_info.value)
