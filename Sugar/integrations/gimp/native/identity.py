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
"""Qualify live document IDs with the owning GIMP process's public Windows creation time."""

from __future__ import annotations

from collections.abc import Callable
import ctypes
from ctypes import wintypes
import sys
from typing import cast


def windows_process_identity(process_id: int) -> str:
    """Read a stable host epoch without changing GIMP state or relying on a recyclable bare PID."""
    if sys.platform == "win32":
        if type(process_id) is not int or process_id < 1:
            raise ValueError("A positive GIMP host process identity is required.")
        library = ctypes.WinDLL("kernel32", use_last_error=True)
        opening: object = getattr(library, "OpenProcess")
        setattr(opening, "argtypes", [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD])
        setattr(opening, "restype", wintypes.HANDLE)
        reading: object = getattr(library, "GetProcessTimes")
        setattr(
            reading,
            "argtypes",
            [wintypes.HANDLE, *([ctypes.POINTER(wintypes.FILETIME)] * 4)],
        )
        setattr(reading, "restype", wintypes.BOOL)
        closing: object = getattr(library, "CloseHandle")
        setattr(closing, "argtypes", [wintypes.HANDLE])
        setattr(closing, "restype", wintypes.BOOL)
        # ctypes builds its functions dynamically; calls are narrowed only inside this OS adapter.
        opened = cast(Callable[[int, bool, int], object], opening)(
            0x1000, False, process_id
        )
        if type(opened) is not int or opened == 0:
            raise ctypes.WinError(ctypes.get_last_error())
        creation, exit_time, kernel, user = (wintypes.FILETIME() for _ in range(4))
        try:
            get_times = cast(
                Callable[[int, object, object, object, object], int], reading
            )
            if not get_times(
                opened,
                ctypes.byref(creation),
                ctypes.byref(exit_time),
                ctypes.byref(kernel),
                ctypes.byref(user),
            ):
                raise ctypes.WinError(ctypes.get_last_error())
            epoch = (creation.dwHighDateTime << 32) | creation.dwLowDateTime
            return f"{process_id:x}-{epoch:x}"
        finally:
            if not cast(Callable[[int], int], closing)(opened):
                raise ctypes.WinError(ctypes.get_last_error())
    raise RuntimeError("This GIMP companion package requires Windows.")
