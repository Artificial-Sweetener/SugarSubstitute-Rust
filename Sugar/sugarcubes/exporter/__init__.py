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
"""Public API for SugarCubes exporter."""

from __future__ import annotations

from typing import Any, Callable, List, Mapping, Optional

from .graph import analyze_cubes
from .io import write_cube, write_cube_to_path, write_cubes, write_cubes_to_paths
from .serializer import ExportedCube, serialize
from .validation import CubeValidationError, validate

DefinitionResolver = Callable[[str], Mapping[str, Any]]


def export(
    prompt: Mapping[str, Any],
    *,
    workflow: Optional[Mapping[str, Any]] = None,
    workflow_version: Optional[int] = None,
    definition_resolver: Optional[DefinitionResolver] = None,
    default_alias_lookup: Optional[Mapping[str, str]] = None,
    cube_ids: Optional[List[str]] = None,
) -> List[ExportedCube]:
    """Convert a ComfyUI prompt graph into SugarCube artifacts."""

    analysis = analyze_cubes(
        prompt, workflow=workflow, default_alias_lookup=default_alias_lookup
    )
    validate(analysis)

    return serialize(
        analysis,
        workflow=workflow,
        workflow_version=workflow_version,
        definition_resolver=definition_resolver,
        cube_ids=cube_ids,
    )


export_cubes = export

__all__ = [
    "DefinitionResolver",
    "ExportedCube",
    "CubeValidationError",
    "export",
    "export_cubes",
    "write_cube",
    "write_cube_to_path",
    "write_cubes",
    "write_cubes_to_paths",
]
