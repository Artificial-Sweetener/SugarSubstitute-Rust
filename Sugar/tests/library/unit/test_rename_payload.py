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

from sugarcubes.payloads import retarget_cube_payload


def test_retarget_payload_updates_sugarcubes_metadata() -> None:
    payload: dict[str, Any] = {
        "nodes": {
            "1": {
                "class_type": "SugarCubes.CubeInput",
                "inputs": {
                    "cube_id": "local/example-user/old.cube",
                    "default_alias": "Old",
                },
            }
        },
        "layout": {
            "groups": [
                {
                    "title": "Old Group",
                    "sugarcubes": {
                        "schema": 2,
                        "instance_id": "inst-1",
                        "cube_id": "local/example-user/old.cube",
                        "default_alias": "Old",
                        "alias": "Old",
                    },
                }
            ]
        },
    }

    retarget_cube_payload(
        payload,
        previous_cube_id="local/example-user/old.cube",
        target_cube_id="local/example-user/new.cube",
        previous_default_alias="Old",
        target_default_alias="New",
    )

    assert payload["nodes"]["1"]["inputs"]["cube_id"] == "local/example-user/new.cube"
    assert payload["nodes"]["1"]["inputs"]["default_alias"] == "New"
    group = payload["layout"]["groups"][0]
    assert group["sugarcubes"]["cube_id"] == "local/example-user/new.cube"
    assert group["sugarcubes"]["default_alias"] == "New"
    assert group["sugarcubes"]["alias"] == "Old"
    assert group["title"] == "Old Group"


def test_retarget_payload_skips_invalid_entries() -> None:
    payload: dict[str, Any] = {
        "nodes": {"1": {"class_type": "NotSugar", "inputs": {"default_alias": "Old"}}},
        "layout": {"groups": [{"sugarcubes": "nope"}]},
    }

    retarget_cube_payload(
        payload,
        previous_cube_id="local/example-user/old.cube",
        target_cube_id="local/example-user/new.cube",
        previous_default_alias="Old",
        target_default_alias="New",
    )

    assert payload["nodes"]["1"]["inputs"]["default_alias"] == "Old"
