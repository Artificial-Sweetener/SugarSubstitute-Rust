#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Own installed SugarCubes package identity and release metadata."""

from __future__ import annotations

import importlib.metadata
import tomllib
from pathlib import Path

SUGARCUBES_DISTRIBUTION_NAME = "SugarCubes"


def runtime_version() -> str:
    """Return the installed or source-tree SugarCubes release version."""

    pyproject_path = Path(__file__).resolve().parents[1] / "pyproject.toml"
    if pyproject_path.exists():
        metadata = tomllib.loads(pyproject_path.read_text(encoding="utf-8"))
        version = metadata.get("project", {}).get("version")
        if isinstance(version, str) and version.strip():
            return version.strip()
        raise RuntimeError("SugarCubes pyproject.toml does not define a version.")
    try:
        return importlib.metadata.version(SUGARCUBES_DISTRIBUTION_NAME)
    except importlib.metadata.PackageNotFoundError:
        raise RuntimeError("SugarCubes package metadata is unavailable.") from None


__all__ = ["SUGARCUBES_DISTRIBUTION_NAME", "runtime_version"]
