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
"""Model validated native workflows without importing Comfy or presentation code."""

from __future__ import annotations

from dataclasses import dataclass
from dataclasses import field
from typing import Mapping


@dataclass(frozen=True)
class EmbeddedCubeDefinition:
    """Represent one executable Cube definition embedded in a workflow."""

    definition_id: str
    cube_id: str
    cube_version: str
    default_alias: str
    semantic_hash: str
    provenance: Mapping[str, object] | None
    payload: Mapping[str, object]
    document: Mapping[str, object] | None = None
    native_subgraphs: tuple[Mapping[str, object], ...] = ()
    native_node_definitions: Mapping[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class CubeInstance:
    """Represent one marked native subgraph instance and its stable identity."""

    instance_id: str
    node_id: str
    definition_id: str
    cube_id: str
    cube_version: str
    instance_alias: str
    payload: Mapping[str, object]
    execution_mode: int = 0


@dataclass(frozen=True)
class CanonicalWorkflow:
    """Hold an immutable validated projection plus the complete source payload."""

    semantic_hash: str
    definitions: tuple[EmbeddedCubeDefinition, ...]
    instances: tuple[CubeInstance, ...]
    payload: Mapping[str, object]

    def definition_index(self) -> dict[str, EmbeddedCubeDefinition]:
        """Index embedded definitions by their native definition identifier."""

        return {definition.definition_id: definition for definition in self.definitions}
