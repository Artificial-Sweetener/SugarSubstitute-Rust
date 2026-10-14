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
"""Adapt an already validated image path to the installed Windows file manager."""

import os
from pathlib import Path
import subprocess
from typing import Protocol


class ImageFileRevealer(Protocol):
    """Keep desktop operations outside image resolution and renderer policy."""

    @property
    def available(self) -> bool:
        """Report whether a supported desktop file manager is installed."""

    def reveal(self, path: Path) -> None:
        """Select one validated image file in its containing folder."""


class WindowsImageFileRevealer:
    """Launch Explorer with a fixed executable and an argument list containing a qualified path."""

    def __init__(self) -> None:
        """Discover only Windows' installed Explorer rather than a PATH-provided executable."""
        self._executable = Path(os.environ.get("WINDIR", "C:/Windows")) / "explorer.exe"

    @property
    def available(self) -> bool:
        """Disable desktop operations for unsupported platforms and service sessions."""
        return (
            os.name == "nt"
            and os.environ.get("SESSIONNAME", "").casefold() != "services"
            and self._executable.is_absolute()
            and self._executable.is_file()
        )

    def reveal(self, path: Path) -> None:
        """Hand an existing image to Explorer; its reused-window handoff may return zero or one."""
        if not self.available or not path.is_absolute() or not path.is_file():
            raise ValueError("File reveal is unavailable for this image.")
        # The executable is the validated Windows Explorer; the image is passed only as one argument.
        result = subprocess.run(  # noqa: S603
            [str(self._executable), f"/select,{path}"],
            check=False,
            timeout=10,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if result.returncode not in {0, 1}:
            raise OSError("The file manager could not reveal this image.")
