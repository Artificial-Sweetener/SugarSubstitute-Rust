# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Run one bounded stdlib-only Windows CPU fixture in a directly launched base interpreter."""

from __future__ import annotations

import argparse
import ctypes
import json
import os
import time
from ctypes import wintypes


def run(token: str, mask: int) -> None:
    """Acknowledge actual PID/affinity, then burn one Python CPU thread for at most 15 seconds."""
    if len(token) != 32 or any(value not in "0123456789abcdef" for value in token) or mask <= 0:
        raise ValueError("Invalid owned load fixture identity or affinity")
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetCurrentProcess.restype = wintypes.HANDLE
    kernel.SetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.c_size_t]
    kernel.SetProcessAffinityMask.restype = wintypes.BOOL
    kernel.GetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)]
    kernel.GetProcessAffinityMask.restype = wintypes.BOOL
    if not kernel.SetProcessAffinityMask(kernel.GetCurrentProcess(), mask):
        raise ctypes.WinError(ctypes.get_last_error())
    observed, system = ctypes.c_size_t(), ctypes.c_size_t()
    if not kernel.GetProcessAffinityMask(kernel.GetCurrentProcess(), ctypes.byref(observed), ctypes.byref(system)):
        raise ctypes.WinError(ctypes.get_last_error())
    print(json.dumps({"pid": os.getpid(), "token": token, "affinity_mask": observed.value,
                      "maximum_lifetime_seconds": 15}), flush=True)
    deadline = time.perf_counter() + 15
    value = 1
    while time.perf_counter() < deadline:
        for _ in range(16384):
            value = (value * 1664525 + 1013904223) & 0xFFFFFFFF


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--token", required=True)
    parser.add_argument("--affinity-mask", required=True, type=int)
    arguments = parser.parse_args()
    run(arguments.token, arguments.affinity_mask)
