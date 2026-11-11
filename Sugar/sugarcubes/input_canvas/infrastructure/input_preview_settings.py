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
"""Validate settings shared by image-file source previews and native effective mask interpretation."""

from __future__ import annotations
import math
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Literal

Primitive = str | int | float | bool


@dataclass(frozen=True)
class InputPreviewSettings:
    """Identify an installed loader and its authored primitive widget settings."""

    kind: Literal["image", "mask"]
    class_type: str
    field: str
    inputs: Mapping[str, Primitive]


def parse_preview_settings(value: object) -> InputPreviewSettings:
    """Reject dynamic request values before invoking installed node code."""
    if not isinstance(value, dict):
        raise ValueError("Invalid input loader settings.")
    kind: object = value.get("kind", "mask")
    if kind not in ("image", "mask"):
        raise ValueError("Invalid source preview kind.")
    class_type: object = value.get("classType")
    field: object = value.get("field")
    inputs: object = value.get("inputs")
    if (
        not isinstance(class_type, str)
        or not 1 <= len(class_type) <= 256
        or not isinstance(field, str)
        or not 1 <= len(field) <= 256
        or not isinstance(inputs, dict)
        or len(inputs) > 128
    ):
        raise ValueError("Invalid input loader settings.")
    normalized: dict[str, Primitive] = {}
    for name, item in inputs.items():
        if not isinstance(name, str) or not isinstance(item, (str, int, float, bool)):
            raise ValueError("Source preview settings must be primitive widget values.")
        if isinstance(item, float) and not math.isfinite(item):
            raise ValueError("Invalid numeric source setting.")
        if len(name) > 256 or isinstance(item, str) and len(item) > 4096:
            raise ValueError("Source preview settings exceed their size limit.")
        normalized[name] = item
    return InputPreviewSettings(kind, class_type, field, normalized)
