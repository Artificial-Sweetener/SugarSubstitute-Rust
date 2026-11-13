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
"""Recognize serialized graph relationships without mistaking authored lists."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

BINDING_SENTINEL = "@binding"


def contains_runtime_reference(value: Any) -> bool:
    """Return whether one value contains a node link or Cube binding reference."""

    if is_runtime_reference(value):
        return True
    if isinstance(value, list):
        return any(contains_runtime_reference(entry) for entry in value)
    if isinstance(value, Mapping):
        return any(contains_runtime_reference(entry) for entry in value.values())
    return False


def is_runtime_reference(value: Any) -> bool:
    """Return whether one value is a direct node link or Cube binding reference."""

    if not isinstance(value, list) or len(value) != 2:
        return False
    source, slot = value
    if source == BINDING_SENTINEL and isinstance(slot, str):
        return True
    return isinstance(source, (str, int)) and isinstance(slot, int)
