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
"""Materialize exact worktree or staged repository snapshots for validation."""

from __future__ import annotations

import subprocess
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from tempfile import TemporaryDirectory


class SnapshotError(RuntimeError):
    """Report failure to materialize an exact repository snapshot."""


@contextmanager
def repository_snapshot(project_root: Path, *, staged: bool) -> Iterator[Path]:
    """Yield the worktree or an isolated copy of the exact Git index."""

    root = project_root.resolve()
    if not staged:
        yield root
        return
    with TemporaryDirectory(prefix="sugarcubes-architecture-index-") as temp:
        snapshot_root = Path(temp).resolve()
        prefix = f"{snapshot_root}{Path().anchor or ''}"
        if not prefix.endswith(("/", "\\")):
            prefix += "\\"
        completed = subprocess.run(  # noqa: S603
            ["git", "checkout-index", "--all", f"--prefix={prefix}"],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if completed.returncode != 0:
            raise SnapshotError(
                "cannot materialize staged repository snapshot: "
                f"{completed.stderr.strip()}"
            )
        yield snapshot_root


__all__ = ["SnapshotError", "repository_snapshot"]
