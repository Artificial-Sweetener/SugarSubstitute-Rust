#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Install trusted extension Python requirements through the host runtime."""

from __future__ import annotations

import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

_PIP_TIMEOUT_SECONDS = 900
PythonRunner = Callable[[Sequence[str], Path, int], subprocess.CompletedProcess[str]]


@dataclass(frozen=True, slots=True)
class PythonRequirementsResult:
    """Describe one requirements installation subprocess outcome."""

    command: tuple[str, ...]
    return_code: int
    stdout: str
    stderr: str


class DependencyPythonRequirementsInstaller:
    """Install requirements without executing extension-authored install scripts."""

    def __init__(
        self,
        *,
        python_executable: Path | None = None,
        runner: PythonRunner | None = None,
    ) -> None:
        """Initialize the adapter with the selected Comfy Python runtime."""

        self._python_executable = python_executable or Path(sys.executable)
        self._runner = runner or _run_subprocess

    def install(self, requirements_path: Path) -> PythonRequirementsResult:
        """Install one trusted, manifest-selected requirements file."""

        command = (
            str(self._python_executable),
            "-m",
            "pip",
            "install",
            "--disable-pip-version-check",
            "-r",
            str(requirements_path),
        )
        result = self._runner(command, requirements_path.parent, _PIP_TIMEOUT_SECONDS)
        return PythonRequirementsResult(
            command=command,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
        )


def _run_subprocess(
    command: Sequence[str], cwd: Path, timeout_seconds: int
) -> subprocess.CompletedProcess[str]:
    """Run pip through an argument list with bounded captured output."""

    return subprocess.run(
        list(command),
        cwd=str(cwd),
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
        check=False,
    )


__all__ = [
    "DependencyPythonRequirementsInstaller",
    "PythonRequirementsResult",
]
