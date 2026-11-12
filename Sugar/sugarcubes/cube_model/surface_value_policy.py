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
"""Surface value persistence policy for SugarCube documents."""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Mapping, Sequence


def is_volatile_seed_control(control: Any) -> bool:
    """Return whether one surface control represents live seed runtime state."""

    input_name = _read_control_string(control, "input_name")
    return input_name == "seed"


def tracked_surface_control_ids(controls: Sequence[Any]) -> list[str]:
    """Return control ids whose values should be persisted and compared."""

    tracked: list[str] = []
    for control in controls:
        control_id = _read_control_string(control, "control_id")
        if not control_id or is_volatile_seed_control(control):
            continue
        tracked.append(control_id)
    return tracked


def volatile_surface_control_ids(controls: Any) -> set[str]:
    """Return volatile control ids whose values should be omitted."""

    if not isinstance(controls, Sequence) or isinstance(controls, (str, bytes)):
        return set()
    volatile: set[str] = set()
    for control in controls:
        control_id = _read_control_string(control, "control_id")
        if control_id and is_volatile_seed_control(control):
            volatile.add(control_id)
    return volatile


def filter_tracked_surface_values(
    controls: Sequence[Any],
    values: Mapping[str, Any],
) -> dict[str, Any]:
    """Return a copy of persisted values with volatile surface controls removed."""

    volatile_ids = volatile_surface_control_ids(controls)
    return {
        str(control_id): deepcopy(value)
        for control_id, value in values.items()
        if str(control_id) not in volatile_ids
    }


def _read_control_string(control: Any, key: str) -> str:
    """Read a normalized string field from a mapping or dataclass-like control."""

    if isinstance(control, Mapping):
        value = control.get(key)
    else:
        value = getattr(control, key, None)
    return value.strip() if isinstance(value, str) else ""
