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
"""Run strict mypy checks for library code and the ComfyUI package bootstrap."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys


def _run_mypy(arguments: list[str], *, cwd: Path, environment: dict[str, str]) -> int:
    """Run one mypy scope and return its process status."""

    completed = subprocess.run(
        [sys.executable, "-m", "mypy", *arguments],
        cwd=cwd,
        env=environment,
        check=False,
    )
    return completed.returncode


def main() -> int:
    """Check every Python module under its actual import identity."""

    project_root = Path(__file__).resolve().parents[1]
    environment = os.environ.copy()
    environment["MYPYPATH"] = str(project_root / "stubs")

    project_status = _run_mypy(
        ["sugarcubes", "tests", "scripts", "tools", "--no-pretty"],
        cwd=project_root,
        environment=environment,
    )
    if project_status != 0:
        return project_status

    return _run_mypy(
        [
            "-m",
            project_root.name,
            "--config-file",
            str(project_root / "pyproject.toml"),
            "--no-pretty",
        ],
        cwd=project_root.parent,
        environment=environment,
    )


if __name__ == "__main__":
    raise SystemExit(main())
