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
"""Verify the read-only cube-pack audit release gate."""

from __future__ import annotations

from pathlib import Path

import json

from sugarcubes.tools.audit_cube_pack import CubePackAuditor


def test_cube_pack_audit_accepts_aligned_widget_values(tmp_path: Path) -> None:
    """A valid cube produces no release-blocking findings."""

    cube_path = tmp_path / "valid.cube"
    cube_path.write_text(json.dumps(_cube_payload(0.75)), encoding="utf-8")

    findings = CubePackAuditor(_live_definitions()).audit_cube(cube_path)

    assert findings == []


def test_cube_pack_audit_reports_type_compatible_range_corruption(
    tmp_path: Path,
) -> None:
    """The audit catches numeric shifts that ordinary type checks would accept."""

    cube_path = tmp_path / "corrupt.cube"
    cube_path.write_text(json.dumps(_cube_payload(2048)), encoding="utf-8")

    findings = CubePackAuditor(_live_definitions()).audit_cube(cube_path)

    assert len(findings) == 1
    assert "white_point" in findings[0].message
    assert "maximum 1.0" in findings[0].message


def _cube_payload(value: float | int) -> dict[str, object]:
    """Build a canonical cube containing one raw subgraph widget."""

    definition = _live_definitions()["WidgetNode"]
    return {
        "cube_id": "local/example-user/audit.cube",
        "version": "1.0.0",
        "description": "audit fixture",
        "implementation": {
            "nodes": {},
            "inputs": {},
            "outputs": {},
            "layout": {},
            "definitions": {"WidgetNode": definition},
            "subgraphs": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "nodes": [
                        {
                            "id": 1,
                            "type": "WidgetNode",
                            "inputs": [
                                {
                                    "name": "white_point",
                                    "type": "FLOAT",
                                    "widget": {"name": "white_point"},
                                    "link": None,
                                }
                            ],
                            "outputs": [],
                            "widgets_values": [value],
                        }
                    ],
                    "links": [],
                    "inputs": [],
                    "outputs": [],
                }
            ],
        },
        "surface": {"default_flavor_id": "default", "controls": []},
        "flavors": {"authored": [{"id": "default", "name": "Default", "values": {}}]},
    }


def _live_definitions() -> dict[str, object]:
    """Return the authoritative object-info fixture for the audit node."""

    return {
        "WidgetNode": {
            "input": {
                "required": {
                    "white_point": [
                        "FLOAT",
                        {"default": 0.99, "min": 0.02, "max": 1.0},
                    ]
                }
            },
            "input_order": {"required": ["white_point"]},
        }
    }
