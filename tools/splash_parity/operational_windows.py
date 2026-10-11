# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Observe owned Windows splash CPU, visibility, OS move-loop and real monitor DPI behavior."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import math
import subprocess
import time
from ctypes import wintypes
from pathlib import Path

try:
    from .native_windows import MonitorInfo, NativeWindows, has_mascot, write_json
except ImportError:
    from native_windows import MonitorInfo, NativeWindows, has_mascot, write_json


class ProcessMemory(ctypes.Structure):
    """Match PROCESS_MEMORY_COUNTERS on the observer's pointer-width ABI."""

    _fields_ = [("size", wintypes.DWORD), ("page_faults", wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in (
            "peak_working_set", "working_set", "paged_peak", "paged", "nonpaged_peak",
            "nonpaged", "pagefile", "peak_pagefile",
        )
    ]


class ObservationWindows:
    """Borrow native resources for one verified process and close every owned observation handle."""

    def __init__(self, native: NativeWindows) -> None:
        """Declare ABI contracts before requesting process metrics or monitor operations."""
        self.native = native
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.psapi = ctypes.WinDLL("psapi", use_last_error=True)
        self.monitor_callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HANDLE, wintypes.HDC,
                                                   ctypes.POINTER(wintypes.RECT), wintypes.LPARAM)
        for function, arguments, result in (
            (self.kernel.OpenProcess, [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
            (self.kernel.CloseHandle, [wintypes.HANDLE], wintypes.BOOL),
            (self.kernel.GetProcessTimes, [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4, wintypes.BOOL),
            (self.kernel.WaitForSingleObject, [wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            (self.psapi.GetProcessMemoryInfo, [wintypes.HANDLE, ctypes.POINTER(ProcessMemory), wintypes.DWORD], wintypes.BOOL),
            (native.user.GetGuiResources, [wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            (native.user.ShowWindow, [wintypes.HWND, ctypes.c_int], wintypes.BOOL),
            (native.user.IsWindowVisible, [wintypes.HWND], wintypes.BOOL),
            (native.user.EnumDisplayMonitors, [wintypes.HDC, ctypes.POINTER(wintypes.RECT), self.monitor_callback, wintypes.LPARAM], wintypes.BOOL),
            (native.user.SetWindowPos, [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.UINT], wintypes.BOOL),
            (native.user.MonitorFromWindow, [wintypes.HWND, wintypes.DWORD], wintypes.HANDLE),
            (native.user.SendMessageTimeoutW, [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM,
                                              wintypes.UINT, wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)], ctypes.c_size_t),
        ):
            function.argtypes = arguments
            function.restype = result

    def snapshot(self, handle: int) -> dict[str, int]:
        """Observe CPU ticks, resident bytes and native object counts from the same owned process."""
        timestamps = [wintypes.FILETIME() for _ in range(4)]
        if not self.kernel.GetProcessTimes(handle, *(ctypes.byref(value) for value in timestamps)):
            raise ctypes.WinError(ctypes.get_last_error())
        ticks = lambda value: (value.dwHighDateTime << 32) | value.dwLowDateTime
        memory = ProcessMemory()
        memory.size = ctypes.sizeof(memory)
        if not self.psapi.GetProcessMemoryInfo(handle, ctypes.byref(memory), memory.size):
            raise ctypes.WinError(ctypes.get_last_error())
        return {"creation_ticks": ticks(timestamps[0]),
                "cpu_100ns": ticks(timestamps[2]) + ticks(timestamps[3]),
                "working_set_bytes": memory.working_set,
                "gdi_objects": self.native.user.GetGuiResources(handle, 0),
                "user_objects": self.native.user.GetGuiResources(handle, 1)}

    def cpu(self, hwnd: int, pid: int, visibility: str) -> dict[str, object]:
        """Measure an explicit 30-second static interval, then require restored character paint."""
        self.native.client(hwnd, pid)
        handle = self.kernel.OpenProcess(0x1000 | 0x100000, False, pid)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            if visibility != "visible":
                self.native.user.ShowWindow(hwnd, 0 if visibility == "hidden" else 6)
                if visibility == "hidden" and self.native.user.IsWindowVisible(hwnd):
                    raise RuntimeError("Owned static window did not become hidden")
                if visibility == "minimized" and not self.native.user.IsIconic(hwnd):
                    raise RuntimeError("Owned static window did not become minimized")
            before = self.snapshot(handle)
            started = time.perf_counter()
            outcome = self.kernel.WaitForSingleObject(handle, 30000)
            elapsed = time.perf_counter() - started
            if outcome != 0x102:
                raise RuntimeError(f"Owned static process ended or observation failed: wait status {outcome}")
            after = self.snapshot(handle)
            if before["creation_ticks"] != after["creation_ticks"]:
                raise RuntimeError("Owned process identity changed during CPU observation")
            one_core_percent = (after["cpu_100ns"] - before["cpu_100ns"]) / 10_000_000 / elapsed * 100
            self.native.client(hwnd, pid)
            self.native.user.ShowWindow(hwnd, 9)
            deadline = time.perf_counter() + 2
            while True:
                image = self.native.capture(hwnd, pid)
                if has_mascot(image, self.native.client(hwnd, pid).scale):
                    break
                if time.perf_counter() >= deadline:
                    raise RuntimeError("Owned static window did not restore character paint")
                time.sleep(0.005)
            return {"visibility": visibility, "interval_seconds": elapsed,
                    "one_logical_core_percent": one_core_percent, "below_one_percent": one_core_percent < 1,
                    "before": before, "after": after, "restore_paint_observed": True,
                    "scope": "S0 demonstration process; no supervisor/backend/browser aggregation claim"}
        finally:
            if not self.kernel.CloseHandle(handle):
                raise ctypes.WinError(ctypes.get_last_error())

    def monitors(self) -> list[tuple[int, MonitorInfo]]:
        """Enumerate actual monitors without changing display configuration or DPI."""
        monitors = []

        def visit(handle: int, _: int, rect: object, data: int) -> bool:
            info = MonitorInfo()
            info.size = ctypes.sizeof(info)
            if not self.native.user.GetMonitorInfoW(handle, ctypes.byref(info)):
                return False
            monitors.append((handle, info))
            return len(monitors) < 16

        callback = self.monitor_callback(visit)
        if not self.native.user.EnumDisplayMonitors(None, None, callback, 0):
            raise ctypes.WinError(ctypes.get_last_error())
        return monitors

    def dpi(self, hwnd: int, pid: int) -> dict[str, object]:
        """Move the owned window through real work areas and require observed logical geometry."""
        observations = []
        for handle, monitor in self.monitors():
            client = self.native.client(hwnd, pid)
            work = monitor.work
            x = work.left + (work.right - work.left - client.width) // 2
            y = work.top + (work.bottom - work.top - client.height) // 2
            if not self.native.user.SetWindowPos(hwnd, None, x, y, 0, 0, 0x15):
                raise ctypes.WinError(ctypes.get_last_error())
            deadline = time.perf_counter() + 2
            while True:
                current = self.native.client(hwnd, pid)
                expected = math.floor(558 * current.scale + 0.5)
                if current.width == current.height == expected and self.native.user.MonitorFromWindow(hwnd, 2) == handle:
                    break
                if time.perf_counter() >= deadline:
                    raise RuntimeError("Real monitor DPI transition did not preserve 558 logical pixels")
                time.sleep(0.005)
            observations.append({"monitor_handle": handle, "work_area": [work.left, work.top, work.right, work.bottom],
                                 "dpi": current.dpi, "client_device_size": [current.width, current.height]})
        changed = len({observation["dpi"] for observation in observations}) > 1
        return {"mixed_dpi_status": "observed" if changed else "pending_unavailable_mixed_dpi",
                "monitors": observations, "synthetic_dpi_messages": False}

    def move_loop(self, hwnd: int, pid: int) -> dict[str, object]:
        """Exercise the real OS modal move loop through keyboard input; this is not physical dragging."""
        self.native.client(hwnd, pid)
        before = wintypes.RECT()
        if not self.native.user.GetWindowRect(hwnd, ctypes.byref(before)):
            raise ctypes.WinError(ctypes.get_last_error())
        self.native.post(hwnd, pid, 0x0112, 0xF012, 0)
        for key in (39, 39, 40):
            self.native.post(hwnd, pid, 0x0100, key, 1)
            self.native.post(hwnd, pid, 0x0101, key, 0xC0000001)
        self.native.post(hwnd, pid, 0x0100, 13, 1)
        self.native.post(hwnd, pid, 0x0101, 13, 0xC0000001)
        deadline = time.perf_counter() + 2
        while True:
            self.native.client(hwnd, pid)
            after = wintypes.RECT()
            if not self.native.user.GetWindowRect(hwnd, ctypes.byref(after)):
                raise ctypes.WinError(ctypes.get_last_error())
            if (before.left, before.top) != (after.left, after.top):
                break
            if time.perf_counter() >= deadline:
                raise RuntimeError("Owned OS caption move loop did not change the window origin")
            time.sleep(0.005)
        result = ctypes.c_size_t()
        started = time.perf_counter()
        if not self.native.user.SendMessageTimeoutW(hwnd, 0, 0, 0, 2, 250, ctypes.byref(result)):
            raise RuntimeError("Owned window did not respond after its OS move loop")
        return {"mode": "OS keyboard move loop", "physical_mouse_drag_qualified": False,
                "before_origin": [before.left, before.top], "after_origin": [after.left, after.top],
                "response_ms": (time.perf_counter() - started) * 1000}


def main() -> int:
    """Launch one owned static demonstration and retain distinct native operational receipts."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--mode", required=True, choices=("cpu", "dpi", "move-loop"))
    parser.add_argument("--visibility", default="visible", choices=("visible", "hidden", "minimized"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    executable = args.executable.resolve(strict=True)
    native = NativeWindows()
    observer = ObservationWindows(native)
    process = subprocess.Popen([str(executable), "--reduced-motion", "--duration-ms", "45000",
                                "--receipt", str(args.output / "submission.json")],
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        hwnd = native.find_window(process, time.perf_counter() + 5)
        deadline = time.perf_counter() + 5
        while not has_mascot(native.capture(hwnd, process.pid), native.client(hwnd, process.pid).scale):
            if time.perf_counter() >= deadline:
                raise RuntimeError("Operational fixture did not display the owned mascot")
            time.sleep(0.005)
        result = (observer.cpu(hwnd, process.pid, args.visibility) if args.mode == "cpu"
                  else observer.dpi(hwnd, process.pid) if args.mode == "dpi"
                  else observer.move_loop(hwnd, process.pid))
        result.update(pid=process.pid, executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest())
        write_json(args.output / "observation.json", result)
        native.post(hwnd, process.pid, 0x0010, 0, 0)
        process.communicate(timeout=5)
        if process.returncode != 0:
            raise RuntimeError(f"Owned operational fixture exited with status {process.returncode}")
        return 1 if args.mode == "cpu" and not result["below_one_percent"] else 0
    except Exception as error:
        write_json(args.output / "failure.json", {"pid": process.pid, "error": str(error)})
        raise
    finally:
        if process.poll() is None:
            process.terminate()
            process.communicate(timeout=5)


if __name__ == "__main__":
    raise SystemExit(main())
