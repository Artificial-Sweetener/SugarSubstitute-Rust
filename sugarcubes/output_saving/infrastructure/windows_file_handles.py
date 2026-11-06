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
"""Pin Windows directory ancestry without delete sharing while resolving an approved output file."""

from collections.abc import Iterator
from contextlib import contextmanager
import ctypes
from ctypes import wintypes
import os
from pathlib import Path
from typing import BinaryIO


def final_handle_path(handle: int) -> Path:
    """Resolve the actual kernel handle rather than trusting a path checked before open."""
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    query = kernel.GetFinalPathNameByHandleW
    query.argtypes = [wintypes.HANDLE, wintypes.LPWSTR, wintypes.DWORD, wintypes.DWORD]
    query.restype = wintypes.DWORD
    buffer = ctypes.create_unicode_buffer(32768)
    count = int(query(handle, buffer, len(buffer), 0))
    if count == 0 or count >= len(buffer):
        raise OSError(
            ctypes.get_last_error(), "The opened output handle cannot be qualified."
        )
    value = buffer.value
    if value.startswith("\\\\?\\UNC\\"):
        value = "\\\\" + value[8:]
    elif value.startswith("\\\\?\\"):
        value = value[4:]
    return Path(value)


@contextmanager
def open_regular_original(path: Path) -> Iterator[BinaryIO]:
    """Refuse active or future writers while one exact original is inspected through its owned handle."""
    import msvcrt

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    create = kernel.CreateFileW
    create.argtypes = [
        wintypes.LPCWSTR,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.LPVOID,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.HANDLE,
    ]
    create.restype = wintypes.HANDLE
    close = kernel.CloseHandle
    close.argtypes = [wintypes.HANDLE]
    close.restype = wintypes.BOOL
    # GENERIC_READ, FILE_SHARE_READ only, OPEN_EXISTING, OPEN_REPARSE_POINT.
    # Write/delete access is excluded for the entire inspection, including existing writers.
    raw: object = create(str(path), 0x80000000, 0x1, None, 3, 0x00200000, None)
    if not isinstance(raw, int) or raw == ctypes.c_void_p(-1).value:
        raise OSError(
            ctypes.get_last_error(), "The output original is busy or unavailable."
        )
    try:
        descriptor = msvcrt.open_osfhandle(raw, os.O_RDONLY | os.O_BINARY)
    except BaseException:
        if not close(raw):
            raise OSError(
                ctypes.get_last_error(),
                "The output original handle could not be released.",
            )
        raise
    # open_osfhandle transfers the kernel handle to this descriptor; fdopen owns its final close.
    try:
        stream = os.fdopen(descriptor, "rb")
    except BaseException:
        os.close(descriptor)
        raise
    with stream:
        yield stream


@contextmanager
def pin_directory_ancestry(directory: Path) -> Iterator[None]:
    """Prevent checked directory components from being renamed or replaced throughout output I/O."""
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    create = kernel.CreateFileW
    create.argtypes = [
        wintypes.LPCWSTR,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.LPVOID,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.HANDLE,
    ]
    create.restype = wintypes.HANDLE
    information = kernel.GetFileInformationByHandleEx
    information.argtypes = [
        wintypes.HANDLE,
        ctypes.c_int,
        wintypes.LPVOID,
        wintypes.DWORD,
    ]
    information.restype = wintypes.BOOL
    close = kernel.CloseHandle
    close.argtypes = [wintypes.HANDLE]
    close.restype = wintypes.BOOL
    handles: list[int] = []
    try:
        for component in (*reversed(directory.parents), directory):
            # FILE_READ_ATTRIBUTES, READ|WRITE sharing (no DELETE), OPEN_EXISTING,
            # BACKUP_SEMANTICS|OPEN_REPARSE_POINT. A link is inspected rather than followed.
            raw: object = create(str(component), 0x80, 0x3, None, 3, 0x02200000, None)
            if not isinstance(raw, int) or raw == ctypes.c_void_p(-1).value:
                raise OSError(
                    ctypes.get_last_error(), "The output ancestry cannot be pinned."
                )
            handles.append(raw)
            attributes = (wintypes.DWORD * 2)()
            if not information(raw, 9, attributes, ctypes.sizeof(attributes)):
                raise OSError(
                    ctypes.get_last_error(), "The output ancestry cannot be inspected."
                )
            if (
                attributes[0] & 0x400
                or not attributes[0] & 0x10
                or final_handle_path(raw) != component
            ):
                raise ValueError(
                    "The output ancestry contains a linked or replaced directory."
                )
        yield
    finally:
        failure: OSError | None = None
        for handle in reversed(handles):
            if not close(handle) and failure is None:
                failure = OSError(
                    ctypes.get_last_error(),
                    "The pinned output ancestry could not be released.",
                )
        if failure is not None:
            raise failure
