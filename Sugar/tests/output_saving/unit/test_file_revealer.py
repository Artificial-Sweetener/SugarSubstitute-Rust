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
"""Verify the Windows file-manager boundary never accepts arbitrary commands or missing files."""

from pathlib import Path
import subprocess

import pytest

from sugarcubes.output_saving.infrastructure.file_revealer import (
    WindowsImageFileRevealer,
)


class InstalledExplorer(WindowsImageFileRevealer):
    """Substitute desktop availability while keeping the actual argument-list handoff."""

    @property
    def available(self) -> bool:
        """Allow the proof to run independently of its operating-system desktop session."""
        return True


def test_explorer_uses_fixed_executable_and_one_qualified_image_argument(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Record only subprocess execution and require an existing absolute image path."""
    monkeypatch.setenv("WINDIR", str(tmp_path))
    calls: list[list[str]] = []

    def run(
        args: list[str], *, check: bool, timeout: float, stdout: int, stderr: int
    ) -> subprocess.CompletedProcess[bytes]:
        """Represent the external file-manager process without opening windows during automated checks."""
        calls.append(args)
        assert check is False
        assert timeout == 10
        return subprocess.CompletedProcess(args, 1)

    monkeypatch.setattr(subprocess, "run", run)
    image = tmp_path / "a space, image.png"
    image.write_bytes(b"pixels")
    revealer = InstalledExplorer()
    revealer.reveal(image)
    assert calls == [[str(tmp_path / "explorer.exe"), f"/select,{image}"]]
    for missing in [Path("relative.png"), tmp_path / "missing.png"]:
        with pytest.raises(ValueError):
            revealer.reveal(missing)
    assert len(calls) == 1
