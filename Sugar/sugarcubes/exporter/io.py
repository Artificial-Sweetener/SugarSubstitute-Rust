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
"""File I/O helpers for SugarCubes exporter."""

from __future__ import annotations

import json
from pathlib import Path
from typing import List, Mapping, Sequence

from ..cube_model import (
    CubeIdentityError,
    apply_cube_identity_projection,
    parse_canonical_cube_id,
    suggest_canonical_cube_path,
)

from .serializer import ExportedCube


def write_cubes_to_paths(
    cube_targets: Sequence[tuple[ExportedCube, Path | str]],
    *,
    overwrite: bool = False,
) -> List[Mapping[str, str]]:
    """Persist exported cubes to explicit target paths and return a summary."""

    saved: List[Mapping[str, str]] = []
    for exported, target_path in cube_targets:
        saved.append(
            write_cube_to_path(
                exported,
                target_path,
                overwrite=overwrite,
            )
        )
    return saved


def write_cube_to_path(
    exported: ExportedCube,
    target_path: Path | str,
    *,
    overwrite: bool = False,
) -> Mapping[str, str]:
    """Persist one exported cube to an explicit path and return a summary."""

    resolved_target = Path(target_path)
    resolved_target.parent.mkdir(parents=True, exist_ok=True)

    if resolved_target.exists():
        if not overwrite:
            raise FileExistsError(
                f"Cube '{exported.default_alias}' already exists at {resolved_target}"
            )

    apply_cube_identity_projection(exported.cube)
    with resolved_target.open("w", encoding="utf-8") as handle:
        json.dump(exported.cube, handle, indent=2)
        handle.write("\n")

    return {
        "default_alias": exported.default_alias,
        "path": str(resolved_target),
        "filename": resolved_target.name,
    }


def write_cubes(
    cubes: Sequence[ExportedCube],
    directory: Path | str,
    *,
    overwrite: bool = False,
) -> List[Mapping[str, str]]:
    """Persist exported cubes to disk and return a summary."""

    base_dir = Path(directory)
    cube_targets = [
        (exported, base_dir / _default_filename(exported)) for exported in cubes
    ]
    return write_cubes_to_paths(
        cube_targets,
        overwrite=overwrite,
    )


def write_cube(
    exported: ExportedCube,
    directory: Path | str,
    *,
    overwrite: bool = False,
) -> Mapping[str, str]:
    """Persist a single exported cube to disk and return a summary."""

    base_dir = Path(directory)
    target_path = base_dir / _default_filename(exported)
    return write_cube_to_path(
        exported,
        target_path,
        overwrite=overwrite,
    )


def _default_filename(exported: ExportedCube) -> str:
    """Build the managed filename for a serialized cube."""

    if isinstance(exported.cube, Mapping):
        raw_id = exported.cube.get("cube_id")
        if isinstance(raw_id, str) and raw_id.strip():
            try:
                parsed = parse_canonical_cube_id(raw_id.strip())
            except CubeIdentityError:
                pass
            else:
                return Path(parsed.path).name
    return suggest_canonical_cube_path(exported.default_alias)
