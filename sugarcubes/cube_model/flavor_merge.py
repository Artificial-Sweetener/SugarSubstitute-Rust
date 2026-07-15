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
"""Authored flavor merge helpers for implementation-save workflows."""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Mapping

from .document import CubeDocument
from .surface_value_policy import tracked_surface_control_ids


def preserve_authored_flavors_for_implementation_save(
    existing: CubeDocument,
    exported: CubeDocument,
) -> CubeDocument:
    """Return the exported cube with authored flavor values reconciled from disk.

    Implementation save replaces the authoritative graph and surface schema. The
    exported surface order owns the saved value order; existing authored flavors
    only supply values for matching controls.
    """

    exported_control_ids = tracked_surface_control_ids(exported.surface.controls)
    exported_default_values = _read_default_values(exported)
    existing_default_values = _read_default_values(existing)

    authored = [
        {
            "id": "default",
            "name": "Default",
            "values": _merge_control_values(
                existing_default_values,
                exported_default_values,
                exported_control_ids,
            ),
        }
    ]

    for existing_flavor in existing.flavors.authored:
        if existing_flavor.id == "default":
            continue
        authored.append(
            {
                "id": existing_flavor.id,
                "name": existing_flavor.name,
                "values": _merge_control_values(
                    existing_flavor.values,
                    exported_default_values,
                    exported_control_ids,
                ),
            }
        )

    payload = exported.to_dict()
    payload["flavors"]["authored"] = authored
    return CubeDocument.from_dict(payload)


def _read_default_values(document: CubeDocument) -> Mapping[str, Any]:
    """Read the default authored flavor values from a validated cube document."""

    for flavor in document.flavors.authored:
        if flavor.id == "default":
            return flavor.values
    return {}


def _merge_control_values(
    existing_values: Mapping[str, Any],
    exported_default_values: Mapping[str, Any],
    exported_control_ids: list[str],
) -> dict[str, Any]:
    """Filter a flavor to the exported surface and seed new controls from export."""

    values: dict[str, Any] = {}
    for control_id in exported_control_ids:
        if control_id in existing_values:
            values[control_id] = deepcopy(existing_values[control_id])
        elif control_id in exported_default_values:
            values[control_id] = deepcopy(exported_default_values[control_id])
    return values
