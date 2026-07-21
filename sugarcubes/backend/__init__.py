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
"""Expose backend package metadata without owning host integration state."""

from __future__ import annotations

import importlib.metadata
import tomllib
from pathlib import Path

_DISTRIBUTION_NAME = "SugarCubes"


def _runtime_version() -> str:
    """Return the installed SugarCubes version from canonical project metadata."""

    pyproject_path = Path(__file__).resolve().parents[2] / "pyproject.toml"
    if pyproject_path.exists():
        metadata = tomllib.loads(pyproject_path.read_text(encoding="utf-8"))
        version = metadata.get("project", {}).get("version")
        if isinstance(version, str) and version.strip():
            return version
        raise RuntimeError("SugarCubes pyproject.toml does not define a version.")
    try:
        return importlib.metadata.version(_DISTRIBUTION_NAME)
    except importlib.metadata.PackageNotFoundError:
        raise RuntimeError("SugarCubes package metadata is unavailable.") from None


__version__ = _runtime_version()

__all__ = ["__version__"]
