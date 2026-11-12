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
"""Own the durable file primitives the install format builds on.

Every write goes to a sibling temporary file that is flushed to disk and then
renamed over the destination, so a power cut leaves either the old or the new
bytes, never a torn file.
"""

from __future__ import annotations

import hashlib
import os
import secrets
import shutil
import stat
from pathlib import Path

_FILE_ATTRIBUTE_REPARSE_POINT = 0x400
_COPY_CHUNK_BYTES = 1024 * 1024


def is_link_or_reparse_point(path: Path) -> bool:
    """Return whether a path is a symbolic link, junction or other reparse point."""

    try:
        status = os.lstat(path)
    except FileNotFoundError:
        return False
    if stat.S_ISLNK(status.st_mode):
        return True
    attributes = getattr(status, "st_file_attributes", 0)
    return bool(attributes & _FILE_ATTRIBUTE_REPARSE_POINT)


def sha256_file(path: Path) -> str:
    """Return the lowercase SHA-256 of one regular file."""

    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(_COPY_CHUNK_BYTES):
            digest.update(chunk)
    return digest.hexdigest()


def _temporary_sibling(destination: Path) -> Path:
    """Return an unpredictable temporary name beside one destination."""

    return destination.with_name(f".{destination.name}.{secrets.token_hex(6)}.partial")


def durable_replace_bytes(destination: Path, content: bytes) -> None:
    """Atomically replace one file with bytes that have reached the disk."""

    temporary = _temporary_sibling(destination)
    try:
        with temporary.open("xb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def durable_replace_text(destination: Path, text: str) -> None:
    """Atomically replace one UTF-8 text file without newline translation."""

    durable_replace_bytes(destination, text.encode("utf-8"))


def durable_copy(source: Path, destination: Path) -> None:
    """Atomically copy one regular file, keeping its modification time."""

    temporary = _temporary_sibling(destination)
    try:
        with source.open("rb") as reader, temporary.open("xb") as writer:
            shutil.copyfileobj(reader, writer, _COPY_CHUNK_BYTES)
            writer.flush()
            os.fsync(writer.fileno())
        shutil.copystat(source, temporary)
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def remove_file(path: Path) -> None:
    """Remove one file or link, clearing a read-only attribute first if needed."""

    try:
        path.unlink()
    except FileNotFoundError:
        return
    except PermissionError:
        os.chmod(path, stat.S_IWRITE | stat.S_IREAD)
        path.unlink()


def remove_tree(path: Path) -> None:
    """Remove a directory this package created, refusing to follow links."""

    if not path.exists() and not path.is_symlink():
        return
    if is_link_or_reparse_point(path):
        remove_file(path) if path.is_symlink() else os.rmdir(path)
        return
    shutil.rmtree(path, onexc=_clear_read_only_and_retry)


def _clear_read_only_and_retry(
    function: object, path: str, error: BaseException
) -> None:
    """Retry one removal after clearing the read-only attribute Windows may set."""

    if not callable(function) or not isinstance(error, PermissionError):
        raise error
    os.chmod(path, stat.S_IWRITE | stat.S_IREAD)
    function(path)


__all__ = [
    "durable_copy",
    "durable_replace_bytes",
    "durable_replace_text",
    "is_link_or_reparse_point",
    "remove_file",
    "remove_tree",
    "sha256_file",
]
