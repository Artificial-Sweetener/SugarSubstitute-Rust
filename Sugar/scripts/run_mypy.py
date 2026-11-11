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

from collections.abc import Callable
import importlib.metadata
import os
from pathlib import Path
import subprocess
import sys
import tomllib

from packaging.requirements import Requirement

# Formatters are never imported by the code mypy checks, so their versions cannot change its result.
_NOT_IMPORTED_BY_CHECKED_CODE = frozenset({"black", "ruff"})


def installed_version(distribution: str) -> str | None:
    """Return the installed version of one distribution, or ``None`` when it is absent."""

    try:
        return importlib.metadata.version(distribution)
    except importlib.metadata.PackageNotFoundError:
        return None


def declared_requirements(pyproject: Path) -> list[Requirement]:
    """Return the runtime and development requirements whose packages the checked code imports."""

    declared = tomllib.loads(pyproject.read_text(encoding="utf-8"))["project"]
    texts = [*declared["dependencies"], *declared["optional-dependencies"]["dev"]]
    requirements = [Requirement(text) for text in texts]
    return [
        requirement
        for requirement in requirements
        if requirement.name.lower() not in _NOT_IMPORTED_BY_CHECKED_CODE
    ]


def unsatisfied_requirements(
    requirements: list[Requirement],
    version_of: Callable[[str], str | None] = installed_version,
) -> list[str]:
    """Describe each requirement the running interpreter's installed packages do not satisfy.

    mypy resolves types from the packages installed beside it, so an interpreter that lacks a declared
    dependency or carries an older one reports type errors that no source change can fix.
    """

    unsatisfied: list[str] = []
    for requirement in requirements:
        version = version_of(requirement.name)
        if version is None:
            unsatisfied.append(f"{requirement} (not installed)")
        elif not requirement.specifier.contains(version, prereleases=True):
            unsatisfied.append(f"{requirement} (installed {version})")
    return unsatisfied


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
    unsatisfied = unsatisfied_requirements(
        declared_requirements(project_root / "pyproject.toml"), installed_version
    )
    if unsatisfied:
        print(
            f"{sys.executable} does not provide the dependencies pyproject.toml declares, so mypy "
            "would report type errors that no source change can fix:",
            file=sys.stderr,
        )
        for description in unsatisfied:
            print(f"  {description}", file=sys.stderr)
        print('Install them with: python -m pip install -e ".[dev]"', file=sys.stderr)
        return 2
    environment = os.environ.copy()
    environment["MYPYPATH"] = str(project_root / "stubs")

    project_status = _run_mypy(
        [
            "sugarcubes",
            "sugarcubes_model_roots",
            "integrations/gimp/native",
            "integrations/gimp/entry",
            "prestartup_script.py",
            "load_model_roots.py",
            "tests",
            "scripts",
            "tools",
            "--no-pretty",
        ],
        cwd=project_root,
        environment=environment,
    )
    if project_status != 0:
        return project_status

    environment["MYPYPATH"] = os.pathsep.join(
        (str(project_root / "stubs"), str(project_root))
    )
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
