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
import json

from sugarcubes.importer import load_cube, prepare_import


def _write_cube(tmp_path: Path, payload: Any) -> Any:
    path = tmp_path / "demo.cube"
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def _base_current_payload() -> dict[str, Any]:
    return {
        "description": "demo",
        "cube_id": "artificial-sweetener/base-cubes/demo.cube",
        "version": "1.0.0",
        "metadata": {},
        "implementation": {
            "nodes": {"node": {"class_type": "KSampler", "inputs": {}}},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {"KSampler": {}},
            "subgraphs": [],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }


def test_prepare_import_uses_grid_when_layout_missing(tmp_path: Path) -> None:
    payload = _base_current_payload()
    payload["implementation"]["layout"] = {}
    path = _write_cube(tmp_path, payload)

    loaded = load_cube(path)
    prepared = prepare_import(loaded, drop_origin=(10, 20))

    assert prepared.layout is not None
    assert prepared.layout["origin"] == [10.0, 20.0]
    assert "Layout origin missing or invalid; defaulting to [0, 0]" in prepared.warnings
    assert "Layout missing node entry for 'node'" in prepared.warnings


def test_prepare_import_offsets_layout_positions(tmp_path: Path) -> None:
    payload = _base_current_payload()
    payload["implementation"]["layout"] = {
        "origin": [5, 5],
        "ds": {"scale": 1.0, "offset": [0, 0]},
        "nodes": {"node": {"id": "1", "pos": [10, 20], "size": [100, 50]}},
        "markers": {},
    }
    path = _write_cube(tmp_path, payload)

    loaded = load_cube(path)
    prepared = prepare_import(loaded, drop_origin=(10, 20))
    node_entry = prepared.nodes[0]

    assert prepared.layout is not None
    assert node_entry["layout"]["pos"] == [25.0, 45.0]
    assert prepared.layout["origin"] == [15.0, 25.0]
