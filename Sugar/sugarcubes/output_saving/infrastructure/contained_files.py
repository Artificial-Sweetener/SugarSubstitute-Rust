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
"""Qualify approved root ancestry and stable opened files before output bytes cross a boundary."""

from collections.abc import Iterator
from contextlib import contextmanager, nullcontext
import os
from pathlib import Path
import stat
import sys
from typing import BinaryIO


def approved_directory(root: Path) -> None:
    """Require an absolute existing directory whose complete ancestry contains no reparse points."""
    if not root.is_absolute() or ".." in root.parts:
        raise ValueError("An approved output root must be absolute and normalized.")
    for ancestor in (*reversed(root.parents), root):
        _regular_component(ancestor, directory=True)


def contained_file(root: Path, path: Path) -> Path:
    """Reject linked ancestry, nonabsolute roots, traversal and nonregular files without following them."""
    if not root.is_absolute() or not path.is_absolute():
        raise ValueError("Output roots and resolved files must be absolute.")
    if ".." in root.parts or ".." in path.parts or not path.is_relative_to(root):
        raise ValueError("The output is outside its approved root.")
    approved_directory(root)
    relative = path.relative_to(root)
    cursor = root
    for index, part in enumerate(relative.parts):
        cursor /= part
        _regular_component(cursor, directory=index < len(relative.parts) - 1)
    if not relative.parts:
        raise ValueError("An output must name a regular file.")
    return path


def _regular_component(path: Path, *, directory: bool) -> None:
    """Reject all reparse attributes rather than resolving a junction and losing its provenance."""
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
        raise ValueError("The output location contains a linked filesystem component.")
    if not (stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)):
        raise ValueError("The output location is not a regular file or directory.")


@contextmanager
def pinned_directory(directory: Path) -> Iterator[None]:
    """Retain checked directory ancestry while enumerating or opening files below an approved root."""
    approved_directory(directory)
    if sys.platform == "win32":
        from .windows_file_handles import pin_directory_ancestry

        with pin_directory_ancestry(directory):
            yield
    else:
        with nullcontext():
            yield
    approved_directory(directory)


@contextmanager
def scan_directory(directory: Path) -> Iterator[Iterator[os.DirEntry[str]]]:
    """Enumerate the actual owned directory handle without following an exchanged linked path."""
    approved_directory(directory)
    if sys.platform == "win32":
        with pinned_directory(directory), os.scandir(directory) as entries:
            yield entries
    else:
        descriptor = _directory_descriptor(directory)
        try:
            with os.scandir(descriptor) as entries:
                yield entries
        finally:
            os.close(descriptor)


def _directory_descriptor(directory: Path) -> int:
    """Walk Unix directory descriptors with no-follow semantics on every component."""
    descriptor = os.open(
        directory.anchor,
        os.O_RDONLY | int(getattr(os, "O_DIRECTORY")) | int(getattr(os, "O_NOFOLLOW")),
    )
    try:
        for component in directory.parts[1:]:
            child = os.open(
                component,
                os.O_RDONLY
                | int(getattr(os, "O_DIRECTORY"))
                | int(getattr(os, "O_NOFOLLOW")),
                dir_fd=descriptor,
            )
            os.close(descriptor)
            descriptor = child
    except BaseException:
        os.close(descriptor)
        raise
    return descriptor


@contextmanager
def open_contained(root: Path, path: Path) -> Iterator[BinaryIO]:
    """Pin one regular file and validate its actual handle under the same approved ancestry."""
    path = contained_file(root, path)
    if sys.platform == "win32":
        import msvcrt
        from .windows_file_handles import final_handle_path, open_regular_original

        with pinned_directory(path.parent), open_regular_original(path) as stream:
            actual = final_handle_path(msvcrt.get_osfhandle(stream.fileno()))
            contained_file(root, actual)
            _same_open_file(stream, path)
            yield stream
            if final_handle_path(msvcrt.get_osfhandle(stream.fileno())) != actual:
                raise ValueError(
                    "The opened output changed location during inspection."
                )
            contained_file(root, actual)
            _same_open_file(stream, path)
    else:
        # Keep each Unix directory descriptor while traversing it; no component can redirect an open.
        directory = _directory_descriptor(path.parent)
        try:
            descriptor = os.open(
                path.name,
                os.O_RDONLY | int(getattr(os, "O_NOFOLLOW")),
                dir_fd=directory,
            )
        finally:
            os.close(directory)
        with os.fdopen(descriptor, "rb") as stream:
            _same_open_file(stream, path)
            yield stream
            contained_file(root, path)
            _same_open_file(stream, path)


def _same_open_file(stream: BinaryIO, path: Path) -> None:
    """Refuse a replaced path or a special handle while the original remains pinned."""
    opened = os.fstat(stream.fileno())
    current = path.stat(follow_symlinks=False)
    if not stat.S_ISREG(opened.st_mode) or (opened.st_dev, opened.st_ino) != (
        current.st_dev,
        current.st_ino,
    ):
        raise ValueError("The output file changed while it was being inspected.")
