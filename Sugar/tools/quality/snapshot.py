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
"""Attach installed build dependencies to an exact source snapshot without copying them."""

from __future__ import annotations

import os
import subprocess
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

from tools.architecture.snapshot import repository_snapshot


@contextmanager
def quality_snapshot(root: Path, *, staged: bool) -> Iterator[Path]:
    """Run staged checks against indexed files and preserve the installed dependency tree."""

    with repository_snapshot(root, staged=staged, package_name=root.name) as snapshot:
        if not staged:
            yield snapshot
            return
        dependencies = root / "node_modules"
        link = snapshot / "node_modules"
        if not dependencies.is_dir():
            raise ValueError(
                "Install repository dependencies before staged validation."
            )
        if os.name == "nt":
            subprocess.run(
                ["cmd.exe", "/c", "mklink", "/J", str(link), str(dependencies)],
                check=True,
                capture_output=True,
            )
        else:
            link.symlink_to(dependencies, target_is_directory=True)
        try:
            yield snapshot
        finally:
            # Remove the attachment itself before temporary-tree cleanup can traverse dependencies.
            if os.name == "nt":
                link.rmdir()
            else:
                link.unlink()
