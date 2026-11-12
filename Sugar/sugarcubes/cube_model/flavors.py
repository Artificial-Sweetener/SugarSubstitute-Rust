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
"""Authored flavor domain types for canonical SugarCube documents."""

from __future__ import annotations

import re
from copy import deepcopy
from dataclasses import dataclass
from typing import Any

_FLAVOR_SLUG_RE = re.compile(r"[^0-9a-z_-]+")


@dataclass(frozen=True)
class AuthoredFlavor:
    """Represent one authored face-value preset shipped with a cube."""

    id: str
    name: str
    values: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-ready authored flavor payload."""

        return {
            "id": self.id,
            "name": self.name,
            "values": deepcopy(self.values),
        }


@dataclass(frozen=True)
class AuthoredFlavorSet:
    """Represent the authored flavors section of a canonical cube."""

    authored: tuple[AuthoredFlavor, ...]

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-ready authored flavor set payload."""

        return {"authored": [flavor.to_dict() for flavor in self.authored]}


def normalize_flavor_id(name: str) -> str:
    """Normalize a flavor display name into the persisted machine id."""

    cleaned = _FLAVOR_SLUG_RE.sub("_", name.strip().lower()).strip("_")
    return cleaned or "flavor"


def dedupe_flavor_id(flavor_id: str, used_ids: set[str]) -> str:
    """Apply the repository numeric-suffix convention to a flavor id."""

    if flavor_id not in used_ids:
        used_ids.add(flavor_id)
        return flavor_id
    suffix = 2
    while True:
        candidate = f"{flavor_id}_{suffix}"
        if candidate not in used_ids:
            used_ids.add(candidate)
            return candidate
        suffix += 1
