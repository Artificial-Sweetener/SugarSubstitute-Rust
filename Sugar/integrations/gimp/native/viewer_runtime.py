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
"""Verify the packaged viewer's runtime before admitting native resources or opening its window."""

from __future__ import annotations

from collections.abc import Mapping
import json
import logging
from pathlib import Path
import subprocess

_logger = logging.getLogger(__name__)


def verify_viewer_runtime(executable: Path, environment: Mapping[str, str]) -> None:
    """Query the viewer's dependency adapter without importing its framework into GIMP.

    The viewer owns platform-specific dependency detection and its actionable message.
    GIMP uses the same process environment for this probe and the actual window.
    """
    try:
        result = subprocess.run(
            [str(executable), "--check-runtime"],
            capture_output=True,
            env=dict(environment),
            timeout=10,
            check=False,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RuntimeError(
            "SugarCubes could not verify its companion runtime. Check GIMP's Error Console for diagnostics."
        ) from error
    if len(result.stdout) > 8192 or len(result.stderr) > 8192:
        raise RuntimeError(
            "The SugarCubes companion returned excessive startup diagnostics."
        )
    try:
        value: object = json.loads(result.stdout)
    except (ValueError, UnicodeDecodeError) as error:
        raise RuntimeError(
            "The SugarCubes companion returned an invalid runtime report."
        ) from error
    if not isinstance(value, dict):
        raise RuntimeError(
            "The SugarCubes companion returned an invalid runtime report."
        )
    status: object = value.get("status")
    version: object = value.get("version")
    if (
        result.returncode == 0
        and status == "ready"
        and isinstance(version, str)
        and 0 < len(version) <= 128
    ):
        return
    message: object = value.get("message")
    detail: object = value.get("detail")
    if (
        status == "unavailable"
        and isinstance(message, str)
        and 0 < len(message) <= 1024
    ):
        _logger.error(
            "SugarCubes companion runtime is unavailable",
            extra={
                "reason": (
                    detail
                    if isinstance(detail, str) and len(detail) <= 1024
                    else "Runtime probe failed"
                )
            },
        )
        raise RuntimeError(message)
    raise RuntimeError("The SugarCubes companion returned an invalid runtime report.")
