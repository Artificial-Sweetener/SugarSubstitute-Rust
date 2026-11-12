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
"""Adapt current-user Windows DPAPI encryption with explicit pointer-safe foreign calls."""

from __future__ import annotations

import ctypes
from ctypes import wintypes
import sys
from typing import Protocol, cast


class _DataBlob(ctypes.Structure):
    """Keep DPAPI-owned byte buffers separate from Python-managed input storage."""

    cbData: int
    pbData: ctypes._Pointer[ctypes.c_ubyte]
    _fields_ = [
        ("cbData", wintypes.DWORD),
        ("pbData", ctypes.POINTER(ctypes.c_ubyte)),
    ]


class _DpapiFunction(Protocol):
    """Narrow the untyped ctypes call at the Windows foreign-function boundary."""

    def __call__(self, *arguments: object) -> int:
        """Return the BOOL result from a configured DPAPI function."""


def protect_user_data(data: bytes) -> bytes:
    """Encrypt with the current Windows user, forbidding operating-system credential dialogs."""
    return _transform(data, protect=True)


def unprotect_user_data(data: bytes) -> bytes:
    """Decrypt only material accessible to the current Windows user."""
    return _transform(data, protect=False)


def _transform(data: bytes, *, protect: bool) -> bytes:
    """Copy DPAPI results and release their Windows allocation on every completed call."""
    if sys.platform != "win32":
        raise OSError("Windows DPAPI is unavailable on this platform.")
    if not data or len(data) > 64 * 1024:
        raise ValueError("Credential encryption material is invalid.")
    crypt32 = ctypes.WinDLL("crypt32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    function = crypt32.CryptProtectData if protect else crypt32.CryptUnprotectData
    function.argtypes = [
        ctypes.POINTER(_DataBlob),
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.c_void_p,
        wintypes.DWORD,
        ctypes.POINTER(_DataBlob),
    ]
    function.restype = wintypes.BOOL
    kernel32.LocalFree.argtypes = [ctypes.c_void_p]
    kernel32.LocalFree.restype = ctypes.c_void_p
    input_buffer = (ctypes.c_ubyte * len(data)).from_buffer_copy(data)
    input_blob = _DataBlob(len(data), input_buffer)
    output_blob = _DataBlob()
    # ctypes exposes dynamic foreign functions; argtypes/restype fix the reviewed ABI above.
    invoke = cast(_DpapiFunction, function)
    if not invoke(
        ctypes.byref(input_blob),
        None,
        None,
        None,
        None,
        0x01,
        ctypes.byref(output_blob),
    ):
        raise OSError("Windows credential encryption failed.")
    try:
        return ctypes.string_at(output_blob.pbData, output_blob.cbData)
    finally:
        kernel32.LocalFree(output_blob.pbData)
