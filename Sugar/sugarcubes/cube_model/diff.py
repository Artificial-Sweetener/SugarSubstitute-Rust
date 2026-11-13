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
"""Typed change classification helpers for canonical cube documents."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any

from .document import CubeDocument


@dataclass(frozen=True)
class CubeChangeSet:
    """Describe how two canonical cube documents differ."""

    interface_changed: bool
    implementation_changed: bool
    authored_flavor_changed: bool
    cosmetic_changed: bool

    @property
    def has_any_change(self) -> bool:
        """Return whether the documents differ in any tracked dimension."""

        return (
            self.interface_changed
            or self.implementation_changed
            or self.authored_flavor_changed
            or self.cosmetic_changed
        )


def classify_changes(old: CubeDocument, new: CubeDocument) -> CubeChangeSet:
    """Compare two canonical documents across explicit ownership boundaries."""

    interface_changed = _stable_json(old.implementation.inputs) != _stable_json(
        new.implementation.inputs
    ) or _stable_json(old.implementation.outputs) != _stable_json(
        new.implementation.outputs
    )
    implementation_changed = (
        _stable_json(old.implementation.nodes) != _stable_json(new.implementation.nodes)
        or _stable_json(old.implementation.definitions)
        != _stable_json(new.implementation.definitions)
        or _stable_json(old.implementation.subgraphs)
        != _stable_json(new.implementation.subgraphs)
        or _stable_json(old.surface.to_dict()) != _stable_json(new.surface.to_dict())
    )
    authored_flavor_changed = _stable_json(old.flavors.to_dict()) != _stable_json(
        new.flavors.to_dict()
    )
    cosmetic_changed = _stable_json(old.implementation.layout) != _stable_json(
        new.implementation.layout
    )
    return CubeChangeSet(
        interface_changed=interface_changed,
        implementation_changed=implementation_changed,
        authored_flavor_changed=authored_flavor_changed,
        cosmetic_changed=cosmetic_changed,
    )


def _stable_json(value: Any) -> str:
    """Serialize one value deterministically for diffing."""

    return json.dumps(value, sort_keys=True, separators=(",", ":"), default=str)
