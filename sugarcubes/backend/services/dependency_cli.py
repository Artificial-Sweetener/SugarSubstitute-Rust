#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
"""Adapt Comfy CLI subprocess execution for dependency installation."""

from __future__ import annotations

import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from ..responses import BackendError
from .cube_metadata import normalize_metadata_string

_CLI_TIMEOUT_SECONDS = 600
SubprocessRunner = Callable[
    [Sequence[str], Path, int], subprocess.CompletedProcess[str]
]


@dataclass(frozen=True)
class ComfyCliResult:
    """Describe one Comfy CLI invocation used for dependency repair."""

    node_id: str
    requested_version: str
    command: tuple[str, ...]
    return_code: int
    stdout: str
    stderr: str

    def to_payload(self) -> dict[str, Any]:
        """Return a JSON-safe install result payload."""

        return {
            "nodeId": self.node_id,
            "requestedVersion": self.requested_version,
            "command": list(self.command),
            "returnCode": self.return_code,
            "stdout": self.stdout,
            "stderr": self.stderr,
        }


class ComfyCliAdapter:
    """Run Comfy CLI through the selected Comfy workspace Python runtime."""

    def __init__(
        self,
        *,
        python_executable: Path | None = None,
        runner: SubprocessRunner | None = None,
    ) -> None:
        """Initialize the adapter with a runtime and subprocess boundary."""

        self._python_executable = python_executable or Path(sys.executable)
        self._runner = runner or _run_subprocess

    def assert_available(self, workspace_path: Path) -> None:
        """Require `comfy_cli` to be importable in the selected runtime."""

        command = (str(self._python_executable), "-c", "import comfy_cli")
        result = self._runner(command, workspace_path, 30)
        if result.returncode != 0:
            raise BackendError(
                "Comfy CLI is not available in the selected Comfy runtime",
                status=424,
                details={
                    "reason": "missing_comfy_cli",
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                },
            )

    def install_node(
        self,
        *,
        workspace_path: Path,
        node_id: str,
        version: str = "",
    ) -> ComfyCliResult:
        """Install one custom node, requesting an exact version when supplied."""

        normalized_node_id = normalize_metadata_string(node_id)
        if not normalized_node_id:
            raise BackendError("Custom node id is required", status=400)
        normalized_version = normalize_metadata_string(version)
        node_spec = (
            f"{normalized_node_id}@{normalized_version}"
            if normalized_version
            else normalized_node_id
        )
        command = (
            str(self._python_executable),
            "-m",
            "comfy_cli",
            "--workspace",
            str(workspace_path),
            "--skip-prompt",
            "node",
            "install",
            "--exit-on-fail",
            "--mode",
            "remote",
            node_spec,
        )
        result = self._runner(command, workspace_path, _CLI_TIMEOUT_SECONDS)
        return ComfyCliResult(
            node_id=normalized_node_id,
            requested_version=normalized_version,
            command=command,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
        )


def _run_subprocess(
    command: Sequence[str], cwd: Path, timeout_seconds: int
) -> subprocess.CompletedProcess[str]:
    """Run one subprocess through an argument list with captured output."""

    return subprocess.run(
        list(command),
        cwd=str(cwd),
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
        check=False,
    )
