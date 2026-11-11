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

"""Characterize authored-default reconciliation during implementation saves."""

from __future__ import annotations

from typing import Any

from sugarcubes.cube_model import (
    CubeDocument,
    merge_implementation_save_defaults,
)


def _document(
    *,
    controls: list[str],
    default_values: dict[str, Any],
    portrait_values: dict[str, Any] | None = None,
) -> CubeDocument:
    """Build one valid cube document with focused surface values."""

    authored: list[dict[str, Any]] = [
        {"id": "default", "name": "Default", "values": default_values}
    ]
    if portrait_values is not None:
        authored.append(
            {"id": "portrait", "name": "Portrait", "values": portrait_values}
        )
    return CubeDocument.from_dict(
        {
            "cube_id": "local/example-user/demo.cube",
            "version": "1.0.0",
            "implementation": {
                "nodes": {},
                "inputs": {},
                "outputs": {},
                "layout": {},
                "definitions": {},
                "subgraphs": [],
            },
            "surface": {
                "default_flavor_id": "default",
                "controls": [
                    {
                        "control_id": control_id,
                        "symbol": control_id.split(".", 1)[0],
                        "input_name": control_id.split(".", 1)[1],
                        "label": control_id,
                        "class_type": "TestNode",
                        "value_type": "number",
                    }
                    for control_id in controls
                ],
            },
            "flavors": {"authored": authored},
        }
    )


def test_existing_values_win_and_follow_the_exported_surface_order() -> None:
    """Keep existing defaults while the new surface owns ordering."""

    existing = _document(
        controls=["node.b", "node.a"],
        default_values={"node.b": 2, "node.a": 1},
    )
    exported = _document(
        controls=["node.a", "node.b"],
        default_values={"node.a": 10, "node.b": 20},
    )

    merged = merge_implementation_save_defaults(existing, exported)

    assert merged.flavors.authored[0].values == {"node.a": 1, "node.b": 2}


def test_new_controls_use_exported_values_and_removed_controls_are_pruned() -> None:
    """Seed newly exposed controls while dropping controls no longer on the face."""

    existing = _document(
        controls=["node.keep", "node.remove"],
        default_values={"node.keep": 1, "node.remove": 2},
    )
    exported = _document(
        controls=["node.keep", "node.add"],
        default_values={"node.keep": 10, "node.add": 30},
    )

    merged = merge_implementation_save_defaults(existing, exported)

    assert merged.flavors.authored[0].values == {"node.keep": 1, "node.add": 30}


def test_named_authored_flavors_receive_new_control_values_without_overwrite() -> None:
    """Preserve named flavors and seed their newly exposed controls."""

    existing = _document(
        controls=["node.keep"],
        default_values={"node.keep": 1},
        portrait_values={"node.keep": 2},
    )
    exported = _document(
        controls=["node.keep", "node.add"],
        default_values={"node.keep": 10, "node.add": 30},
    )

    merged = merge_implementation_save_defaults(existing, exported)

    assert merged.flavors.authored[0].values == {"node.keep": 1, "node.add": 30}
    assert merged.flavors.authored[1].values == {"node.keep": 2, "node.add": 30}
