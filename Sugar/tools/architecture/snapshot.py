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

import os
import subprocess
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path, PurePosixPath
from tempfile import TemporaryDirectory


class SnapshotError(RuntimeError):
    """Report failure to materialize an exact repository snapshot."""


@contextmanager
def repository_snapshot(
    project_root: Path, *, staged: bool, package_name: str | None = None
) -> Iterator[Path]:
    """Yield the worktree or an isolated copy of the project's exact Git index.

    The project may be the repository root or a tracked folder inside a larger repository.
    Only the project's own indexed files are materialized, and the snapshot root holds them
    exactly as the project root holds them in the worktree.
    """

    root = project_root.resolve()
    if not staged:
        yield root
        return
    with TemporaryDirectory(prefix="sugarcubes-architecture-index-") as temp:
        if package_name is not None and not package_name.isidentifier():
            raise SnapshotError(
                "Execution snapshots require a valid Python package directory name."
            )
        temporary = Path(temp).resolve()
        export_root = temporary / "index"
        export_root.mkdir()
        project_prefix = (
            _git(root, "rev-parse", "--show-prefix").decode("utf-8").strip()
        )
        indexed = _git(root, "ls-files", "--cached", "-z")
        # Git writes each exported entry at its repository-relative path below the prefix.
        _git(
            root,
            "checkout-index",
            "--stdin",
            "-z",
            f"--prefix={export_root}{os.sep}",
            stdin=indexed,
        )
        exported = export_root.joinpath(*PurePosixPath(project_prefix).parts)
        exported.mkdir(parents=True, exist_ok=True)
        snapshot_root = temporary / (package_name or "snapshot")
        exported.rename(snapshot_root)
        yield snapshot_root


def _git(root: Path, *args: str, stdin: bytes | None = None) -> bytes:
    """Run one Git index query or export and fail with its diagnostic."""

    completed = subprocess.run(  # noqa: S603
        ["git", *args],
        cwd=root,
        check=False,
        capture_output=True,
        input=stdin,
    )
    if completed.returncode != 0:
        message = completed.stderr.decode("utf-8", errors="replace").strip()
        raise SnapshotError(f"cannot materialize staged repository snapshot: {message}")
    return completed.stdout


__all__ = ["SnapshotError", "repository_snapshot"]
