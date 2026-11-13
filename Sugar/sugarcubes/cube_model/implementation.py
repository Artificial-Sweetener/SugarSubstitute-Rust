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
"""Implementation domain types for canonical SugarCube documents."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class CubeImplementation:
    """Represent the runnable implementation section of a canonical cube."""

    nodes: dict[str, dict[str, Any]] = field(default_factory=dict)
    inputs: dict[str, Any] = field(default_factory=dict)
    outputs: dict[str, Any] = field(default_factory=dict)
    layout: dict[str, Any] = field(default_factory=dict)
    definitions: dict[str, Any] = field(default_factory=dict)
    subgraphs: list[dict[str, Any]] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        """Return a deep-copied JSON-ready implementation payload."""

        return {
            "nodes": deepcopy(self.nodes),
            "inputs": deepcopy(self.inputs),
            "outputs": deepcopy(self.outputs),
            "layout": deepcopy(self.layout),
            "definitions": deepcopy(self.definitions),
            "subgraphs": deepcopy(self.subgraphs),
        }
