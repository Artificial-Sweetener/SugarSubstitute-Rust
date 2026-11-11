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
"""Prove viewer dependency checks fail before admitting native resources or opening a window."""

from __future__ import annotations

from pathlib import Path
import subprocess
from unittest.mock import Mock

import pytest

from integrations.gimp.native.viewer_runtime import verify_viewer_runtime


def test_ready_runtime_uses_the_exact_viewer_environment(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Check dependencies headlessly using the same admitted environment as the actual viewer."""
    run = Mock(
        return_value=subprocess.CompletedProcess(
            ["viewer.exe"], 0, b'{"status":"ready","version":"140.0"}', b""
        )
    )
    monkeypatch.setattr(subprocess, "run", run)
    executable = Path("viewer.exe")
    environment = {"SUGARCUBES_GIMP_IMAGE_NAME": "Current image"}
    verify_viewer_runtime(executable, environment)
    run.assert_called_once_with(
        [str(executable), "--check-runtime"],
        capture_output=True,
        env=environment,
        timeout=10,
        check=False,
        creationflags=subprocess.CREATE_NO_WINDOW,
    )


def test_missing_runtime_preserves_the_actionable_viewer_message(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Let the viewer own dependency instructions while GIMP retains native failure handling."""
    run = Mock(
        return_value=subprocess.CompletedProcess(
            ["viewer.exe"],
            1,
            b'{"status":"unavailable","message":"Install WebView2, then reopen SugarCubes.","detail":"Missing runtime"}',
            b"",
        )
    )
    monkeypatch.setattr(subprocess, "run", run)
    with pytest.raises(RuntimeError, match="Install WebView2"):
        verify_viewer_runtime(Path("viewer.exe"), {})


@pytest.mark.parametrize(
    "stdout,code",
    [
        (b"not-json", 0),
        (b"[]", 0),
        (b'{"status":"ready","version":"140.0"}', 1),
        (b'{"status":"ready","version":""}', 0),
        (b'{"status":"unavailable","message":17}', 1),
        (b"x" * 8193, 0),
    ],
)
def test_invalid_runtime_reports_do_not_open_the_viewer(
    monkeypatch: pytest.MonkeyPatch,
    stdout: bytes,
    code: int,
) -> None:
    """Reject malformed or inconsistent dependency reports at the process boundary."""
    monkeypatch.setattr(
        subprocess,
        "run",
        Mock(
            return_value=subprocess.CompletedProcess(["viewer.exe"], code, stdout, b"")
        ),
    )
    with pytest.raises(RuntimeError):
        verify_viewer_runtime(Path("viewer.exe"), {})


@pytest.mark.parametrize(
    "error", [OSError("Missing viewer"), subprocess.TimeoutExpired("viewer", 10)]
)
def test_probe_failure_preserves_its_cause(
    monkeypatch: pytest.MonkeyPatch,
    error: Exception,
) -> None:
    """Return bounded startup failures with diagnostic exception context."""
    monkeypatch.setattr(subprocess, "run", Mock(side_effect=error))
    with pytest.raises(RuntimeError, match="could not verify") as result:
        verify_viewer_runtime(Path("viewer.exe"), {})
    assert result.value.__cause__ is error
