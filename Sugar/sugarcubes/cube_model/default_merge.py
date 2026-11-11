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

"""Apply validated default-change decisions to implementation-save documents."""

from __future__ import annotations

from copy import deepcopy
from collections.abc import Iterable
from typing import Any, Mapping

from .default_change_plan import (
    build_default_change_plan,
    declared_control_default,
    is_multiline_or_unknown_text_control,
)
from .document import CubeDocument
from .flavors import AuthoredFlavor
from .surface_value_policy import tracked_surface_control_ids


def merge_implementation_save_defaults(
    existing: CubeDocument | None,
    exported: CubeDocument,
    *,
    overwrite_control_ids: frozenset[str] = frozenset(),
    expected_fingerprint: str = "",
) -> CubeDocument:
    """Return an exported document with one authorized authored-default merge."""

    plan = build_default_change_plan(existing, exported)
    if expected_fingerprint and expected_fingerprint != plan.fingerprint:
        raise ValueError("Cube defaults changed while the save review was open.")
    reviewable_ids = {change.control_id for change in plan.changes}
    unknown_ids = sorted(overwrite_control_ids - reviewable_ids)
    if unknown_ids:
        raise ValueError(
            "Default overwrite decisions reference controls outside the current review: "
            + ", ".join(unknown_ids)
        )

    exported_values = _default_values(exported)
    existing_flavors = existing.flavors.authored if existing is not None else ()
    existing_default = _flavor_values(existing_flavors, "default")
    controls = {control.control_id: control for control in exported.surface.controls}
    tracked_ids = tracked_surface_control_ids(exported.surface.controls)
    authored: list[dict[str, Any]] = [
        {
            "id": "default",
            "name": "Default",
            "values": _merge_values(
                current=existing_default,
                exported_values=exported_values,
                exported=exported,
                controls=controls,
                tracked_ids=tracked_ids,
                overwrite_control_ids=overwrite_control_ids,
            ),
        }
    ]
    for flavor in existing_flavors:
        if flavor.id == "default":
            continue
        authored.append(
            {
                "id": flavor.id,
                "name": flavor.name,
                "values": _merge_values(
                    current=flavor.values,
                    exported_values=exported_values,
                    exported=exported,
                    controls=controls,
                    tracked_ids=tracked_ids,
                    overwrite_control_ids=frozenset(),
                ),
            }
        )

    payload = exported.to_dict()
    payload["flavors"]["authored"] = authored
    return CubeDocument.from_dict(payload)


def _merge_values(
    *,
    current: Mapping[str, Any],
    exported_values: Mapping[str, Any],
    exported: CubeDocument,
    controls: Mapping[str, Any],
    tracked_ids: list[str],
    overwrite_control_ids: frozenset[str],
) -> dict[str, Any]:
    """Merge one flavor in exported surface order using the safe baseline policy."""

    values: dict[str, Any] = {}
    for control_id in tracked_ids:
        if control_id in overwrite_control_ids and control_id in exported_values:
            values[control_id] = deepcopy(exported_values[control_id])
            continue
        if control_id in current:
            values[control_id] = deepcopy(current[control_id])
            continue
        control = controls[control_id]
        if is_multiline_or_unknown_text_control(exported, control):
            values[control_id] = declared_control_default(exported, control)
        elif control_id in exported_values:
            values[control_id] = deepcopy(exported_values[control_id])
    return values


def _default_values(document: CubeDocument) -> Mapping[str, Any]:
    """Read exported default values."""

    return _flavor_values(document.flavors.authored, "default")


def _flavor_values(
    flavors: Iterable[AuthoredFlavor], flavor_id: str
) -> Mapping[str, Any]:
    """Read one flavor's values without exposing collection mechanics."""

    for flavor in flavors:
        if flavor.id == flavor_id:
            return flavor.values
    return {}
