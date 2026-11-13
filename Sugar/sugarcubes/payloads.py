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
"""Payload mutation helpers for SugarCubes."""

from __future__ import annotations

from collections.abc import Mapping, MutableMapping
from typing import Any


def retarget_cube_payload(
    payload: MutableMapping[str, Any],
    *,
    previous_cube_id: str,
    target_cube_id: str,
    previous_default_alias: str,
    target_default_alias: str,
) -> None:
    """Retarget embedded cube identity and display-name references."""

    nodes = payload.get("nodes")
    if isinstance(nodes, Mapping):
        for entry in nodes.values():
            if not isinstance(entry, MutableMapping):
                continue
            class_type = entry.get("class_type")
            if not isinstance(class_type, str):
                continue
            if not class_type.startswith("SugarCubes.Cube"):
                continue
            inputs = entry.get("inputs")
            if not isinstance(inputs, MutableMapping):
                continue
            cube_id = inputs.get("cube_id")
            if isinstance(cube_id, str) and cube_id.strip() == previous_cube_id:
                inputs["cube_id"] = target_cube_id
            default_alias = inputs.get("default_alias")
            if (
                isinstance(default_alias, str)
                and default_alias.strip() == previous_default_alias
            ):
                inputs["default_alias"] = target_default_alias
    layout = payload.get("layout")
    if isinstance(layout, Mapping):
        groups = layout.get("groups")
        if isinstance(groups, list):
            for group in groups:
                if not isinstance(group, MutableMapping):
                    continue
                sugarcubes = group.get("sugarcubes")
                if not isinstance(sugarcubes, Mapping):
                    continue
                updated = dict(sugarcubes)
                cube_id = sugarcubes.get("cube_id")
                if isinstance(cube_id, str) and cube_id.strip() == previous_cube_id:
                    updated["cube_id"] = target_cube_id
                default_alias = sugarcubes.get("default_alias")
                if (
                    isinstance(default_alias, str)
                    and default_alias.strip() == previous_default_alias
                ):
                    updated["default_alias"] = target_default_alias
                if updated != sugarcubes:
                    group["sugarcubes"] = updated
