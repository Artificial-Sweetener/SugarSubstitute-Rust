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
"""Surface domain types for canonical SugarCube documents."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class SurfaceControl:
    """Describe one face-node control that flavors may populate."""

    control_id: str
    symbol: str
    input_name: str
    label: str
    class_type: str
    value_type: str

    def to_dict(self) -> dict[str, str]:
        """Return a JSON-ready control mapping."""

        return {
            "control_id": self.control_id,
            "symbol": self.symbol,
            "input_name": self.input_name,
            "label": self.label,
            "class_type": self.class_type,
            "value_type": self.value_type,
        }


@dataclass(frozen=True)
class CubeSurface:
    """Represent the flavor-backed surface section of a canonical cube."""

    default_flavor_id: str
    controls: tuple[SurfaceControl, ...]

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-ready surface payload."""

        return {
            "default_flavor_id": self.default_flavor_id,
            "controls": [control.to_dict() for control in self.controls],
        }


def infer_value_type(value: Any) -> str:
    """Classify a persisted surface value into a stable coarse type."""

    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return "number"
    if isinstance(value, str):
        return "string"
    if value is None:
        return "null"
    if isinstance(value, list):
        return "array"
    if isinstance(value, dict):
        return "object"
    return "unknown"


def compute_surface_signature(surface: CubeSurface) -> str:
    """Compute a stable surface signature for local-flavor segregation."""

    serialized = json.dumps(surface.to_dict(), sort_keys=True, separators=(",", ":"))
    return hashlib.sha1(serialized.encode("utf-8")).hexdigest()[:12]
