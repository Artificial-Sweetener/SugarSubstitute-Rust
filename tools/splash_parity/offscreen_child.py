# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Launch one test executable with diagnostics, only from an inactive QA desktop."""
import ctypes
import subprocess
import sys
from ctypes import wintypes
from pathlib import Path


def main():
    user = ctypes.WinDLL('user32', use_last_error=True)
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.GetCurrentThreadId.restype = wintypes.DWORD
    user.GetThreadDesktop.argtypes = [wintypes.DWORD]
    user.GetThreadDesktop.restype = wintypes.HANDLE
    user.GetUserObjectInformationW.argtypes = [wintypes.HANDLE, ctypes.c_int,
        ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    name = ctypes.create_unicode_buffer(256)
    needed = wintypes.DWORD()
    desktop = user.GetThreadDesktop(kernel.GetCurrentThreadId())
    if not user.GetUserObjectInformationW(desktop, 2, name, ctypes.sizeof(name), ctypes.byref(needed)):
        raise ctypes.WinError(ctypes.get_last_error())
    if not name.value.startswith('SugarSubstitute-QA-'):
        raise RuntimeError('Diagnostic child must run on an inactive QA desktop')
    executable, directory, output = map(Path, sys.argv[1:])
    if output.resolve().drive.casefold() != 'd:':
        raise RuntimeError('Native diagnostic artifacts belong on D:')
    with output.open('xb') as stream:
        child = subprocess.Popen([str(executable)], cwd=directory, stdout=stream, stderr=stream,
                                 creationflags=subprocess.CREATE_NO_WINDOW)
        raise SystemExit(child.wait())


if __name__ == '__main__':
    try:
        main()
    except Exception:
        import traceback
        output = Path(sys.argv[-1])
        if output.resolve().drive.casefold() == 'd:':
            output.with_suffix('.helper-error.log').write_text(traceback.format_exc())
        raise
