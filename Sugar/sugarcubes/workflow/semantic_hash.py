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
"""Compute deterministic hashes for browser-portable workflow meaning."""

from __future__ import annotations

import hashlib
import json
import math
from typing import Mapping, Sequence


def semantic_hash(payload: Mapping[str, object]) -> str:
    """Hash JSON meaning after Comfy's JavaScript number representation."""

    canonical = _encode_value(payload)
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def _encode_value(value: object) -> str:
    """Encode one JSON-domain value with explicit, collision-safe type tags."""

    if value is None:
        return "n"
    if isinstance(value, bool):
        return "b1" if value else "b0"
    if isinstance(value, (int, float)):
        return _encode_number(value)
    if isinstance(value, str):
        return "s" + json.dumps(value, ensure_ascii=False)
    if isinstance(value, Mapping):
        entries = []
        for key in sorted(value):
            if not isinstance(key, str):
                raise TypeError("Semantic JSON object keys must be strings.")
            entries.append(_encode_value(key) + _encode_value(value[key]))
        return "o" + _encode_sequence(entries)
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return "a" + _encode_sequence([_encode_value(item) for item in value])
    raise TypeError(f"Unsupported semantic JSON value: {type(value).__name__}")


def _encode_number(value: int | float) -> str:
    """Represent numbers as the finite IEEE-754 values JavaScript observes."""

    number = float(value)
    if not math.isfinite(number):
        raise ValueError("Semantic JSON numbers must be finite.")
    if number == 0.0:
        number = 0.0
    return "d" + number.hex()


def _encode_sequence(values: Sequence[str]) -> str:
    """Frame encoded values so adjacent content cannot collide."""

    return "".join(f"{len(value)}:{value}" for value in values)
