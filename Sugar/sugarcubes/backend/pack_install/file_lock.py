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
"""Serialize update work on one pack folder across processes.

Two ComfyUI processes can share one `custom_nodes` folder. The lock is an
operating-system byte-range lock, so the kernel releases it when its process
dies and a crash can never leave a stale lock behind. It is only ever tried,
never waited on: a busy folder is a typed outcome the caller reports.
"""

from __future__ import annotations

import os
import sys
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

from .errors import PackInstallError, PackInstallErrorCode

LOCK_FILE_NAME = ".lock"


@contextmanager
def exclusive_update_lock(updates_root: Path) -> Iterator[None]:
    """Hold the folder's update lock for the duration of the block."""

    updates_root.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(updates_root / LOCK_FILE_NAME, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        _try_lock(descriptor)
        try:
            yield
        finally:
            _unlock(descriptor)
    finally:
        os.close(descriptor)


if sys.platform == "win32":
    import msvcrt

    def _try_lock(descriptor: int) -> None:
        """Take the first byte without waiting."""

        os.lseek(descriptor, 0, os.SEEK_SET)
        try:
            msvcrt.locking(descriptor, msvcrt.LK_NBLCK, 1)
        except OSError as exc:
            raise _busy() from exc

    def _unlock(descriptor: int) -> None:
        """Release the first byte."""

        os.lseek(descriptor, 0, os.SEEK_SET)
        msvcrt.locking(descriptor, msvcrt.LK_UNLCK, 1)

else:
    import fcntl

    def _try_lock(descriptor: int) -> None:
        """Take the whole-file lock without waiting."""

        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as exc:
            raise _busy() from exc

    def _unlock(descriptor: int) -> None:
        """Release the whole-file lock."""

        fcntl.flock(descriptor, fcntl.LOCK_UN)


def _busy() -> PackInstallError:
    """Build the typed outcome for a folder another process is updating."""

    return PackInstallError(
        PackInstallErrorCode.BUSY, "Another ComfyUI is updating this pack."
    )


__all__ = ["LOCK_FILE_NAME", "exclusive_update_lock"]
