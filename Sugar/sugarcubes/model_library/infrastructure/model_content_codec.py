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
"""Validate model-level maturity evidence independently of preview-image policy."""

from __future__ import annotations


def optional_nsfw(value: object) -> bool | None:
    """Preserve absent classification as unknown and reject truthy non-boolean values."""
    if value is None or isinstance(value, bool):
        return value
    raise ValueError("Invalid model NSFW classification.")


def optional_nsfw_level(value: object) -> int | None:
    """Retain the provider's raw aggregate rating without interpreting it as a scalar score."""
    if value is None:
        return None
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value < 0
        or value > 2**31 - 1
    ):
        raise ValueError("Invalid model NSFW level.")
    return value
