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
"""Load and normalize Cube artifacts from disk."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Mapping

from ..cube_model import (
    CubeDocument,
    CubeSchemaError,
    looks_like_legacy_cube_payload,
)
from .coercion import _coerce_str
from .document_loader import load_cube_document
from .models import CubeImportError, LoadedCube


def load_cube(path: Path | str) -> LoadedCube:
    """Load and validate a cube JSON file from disk."""

    cube_path = Path(path)
    if not cube_path.exists():
        raise CubeImportError(f"Cube file not found: {cube_path}")

    try:
        with cube_path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError as exc:
        raise CubeImportError(
            "Cube file is not valid JSON",
            details={"path": str(cube_path)},
        ) from exc

    if not isinstance(payload, Mapping):
        raise CubeImportError("Cube root must be a JSON object")

    if looks_like_legacy_cube_payload(payload):
        cube_id = _coerce_str(payload.get("cube_id")) or ""
        raise CubeImportError(
            "Legacy cube format is unsupported. Run scripts/migrate_legacy_cubes.py.",
            details={
                "legacy": True,
                "cube_id": cube_id,
                "path": str(cube_path),
            },
        )

    try:
        document = CubeDocument.from_dict(payload)
    except CubeSchemaError as exc:
        raise CubeImportError(str(exc), details={"path": str(cube_path)}) from exc

    return load_cube_document(document, artifact_name=str(cube_path))
