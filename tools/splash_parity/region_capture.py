# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Observe owned physical display pixels with one shared security and DPI lifetime.

Whole observation brackets include every ownership, native and immutable-copy
cost. Driver calls can be ininterruptible despite finite native wait settings.
"""
from __future__ import annotations
import ctypes
import os
from dataclasses import asdict
from typing import Any
try:
    from .region_records import Region, Geometry, RegionSample, RegionPending, integer, validate_record
    from .gdi_storage import GdiCopyStorage
except ImportError:
    from region_records import Region, Geometry, RegionSample, RegionPending, integer, validate_record
    from gdi_storage import GdiCopyStorage

_PER_MONITOR_AWARE_V2 = -4
# Complete recovery ownership includes storage, process and DPI obligations.
# In particular, a failed unbound __enter__ must not lose its recovery owner.
_CAPTURE_OWNERS = {}


class RegionCapture:
    """Own guards and a single explicit copy backend, with no backend fallback.

    Creation identity is supplied by the verified process owner. PMv2 remains
    active until all storage/process handles have actually been released. Failed
    close remains owned and can be explicitly retried on the creating thread.
    """
    def __init__(self, hwnd, pid, region, *, creation_ticks, flush=False, boundary=None, storage=None):
        integer(hwnd, 1, (1 << (ctypes.sizeof(ctypes.c_void_p) * 8)) - 1, "HWND")
        integer(pid, 1, 0xffffffff, "PID")
        integer(creation_ticks, 1, 0xffffffffffffffff, "process creation ticks")
        if not isinstance(region, Region) or type(flush) is not bool:
            raise ValueError("Capture requires a validated Region and boolean flush option")
        if storage is not None and flush:
            raise ValueError("Flush is configured by the explicit GDI storage owner")
        self.hwnd, self.pid, self.region, self.creation_ticks = hwnd, pid, region, creation_ticks
        self._native = boundary if boundary is not None else _Win32Boundary()
        self._storage = storage if storage is not None else GdiCopyStorage(flush=flush)
        self._process = self._geometry = self._thread = self._previous_dpi = None
        self._frequency = 0
        self._started = self._ready = False
        self.last_attempt = None

    def __enter__(self):
        if self._started:
            raise RuntimeError("A region capture lifetime cannot be entered again")
        if len(_CAPTURE_OWNERS) >= 8:
            raise RuntimeError("Unsettled captures exhausted the bounded recovery inventory")
        self._started = True
        _CAPTURE_OWNERS[id(self)] = self
        try:
            self._thread = self._native.thread_id()
            self._previous_dpi = self._native.set_dpi_context(_PER_MONITOR_AWARE_V2)
            if not self._previous_dpi:
                raise RuntimeError("Native thread DPI context could not be established")
            self._verify_dpi()
            self._frequency = self._native.frequency()
            integer(self._frequency, 1, 0x7fffffffffffffff, "QPC frequency")
            self._process = self._native.open_process(self.pid)
            self._geometry = self._verify()
            self._storage.open(self._geometry, self.region, self._frequency, self._thread, self._native.ticks)
            self._ready = True
            return self
        except BaseException as primary:
            self.last_attempt = {"schema_version": 2, "backend": self._storage.backend, "operation": "enrollment",
                                 "accepted": False, "failure": str(primary),
                                 "backend_evidence": getattr(self._storage, "last_creation", None)}
            try:
                self.close()
            except BaseException as cleanup:
                self.last_attempt["cleanup_evidence"] = getattr(self._storage, "last_cleanup", None)
                raise BaseExceptionGroup("Region capture construction and cleanup failed", [primary, cleanup])
            raise

    def _verify_dpi(self) -> None:
        if not self._native.dpi_equal(self._native.dpi_context(), _PER_MONITOR_AWARE_V2):
            raise RuntimeError("Region capture requires its owned Per Monitor v2 thread DPI context")

    def _verify(self) -> Geometry:
        native = self._native
        if native.thread_id() != self._thread:
            raise RuntimeError("Region capture crossed its owning native thread")
        self._verify_dpi()
        if not native.process_alive(self._process) or native.creation(self._process) != self.creation_ticks:
            raise RuntimeError("Retained process ended or its verified creation identity differs")
        if native.window_pid(self.hwnd) != self.pid or native.root(self.hwnd) != self.hwnd:
            raise RuntimeError("Capture HWND is not the owned process's root window")
        if not native.visible(self.hwnd) or native.minimized(self.hwnd):
            raise RuntimeError("Capture client is hidden or minimized")
        foreground = native.foreground()
        if not foreground or native.root(foreground) != self.hwnd:
            raise RuntimeError("Capture client is not the owned foreground root")
        geometry = native.geometry(self.hwnd)
        if self._geometry is not None and geometry != self._geometry:
            raise RuntimeError("Owned capture geometry or DPI moved after enrollment")
        region = self.region
        if geometry.width <= 0 or geometry.height <= 0 or geometry.dpi <= 0:
            raise RuntimeError("Capture client has invalid physical geometry")
        if region.x + region.width > geometry.width or region.y + region.height > geometry.height:
            raise RuntimeError("Capture region extends outside the physical client")
        left, top = geometry.screen_x + region.x, geometry.screen_y + region.y
        right, bottom = left + region.width, top + region.height
        if not all(-0x80000000 <= value <= 0x7fffffff for value in (left, top, right - 1, bottom - 1)):
            raise RuntimeError("Capture screen region exceeds signed native coordinates")
        for other in native.above(self.hwnd):
            if native.visible(other) and not native.minimized(other):
                x1, y1, x2, y2 = native.bounds(other)
                if left < x2 and right > x1 and top < y2 and bottom > y1:
                    raise RuntimeError("A native window above the owned client occludes the region")
        for y in range(top, bottom):
            for x in range(left, right):
                hit = native.hit(x, y)
                if not hit or native.root(hit) != self.hwnd or native.window_pid(hit) != self.pid:
                    raise RuntimeError("A capture pixel does not hit the owned root or native child")
        return geometry

    def sample(self, *, deadline_ticks=None, minimum_present_ticks=0):
        """Perform one ready/pending attempt under original absolute QPC bounds.

        Standalone GDI callers retain their public unbounded sample behavior;
        explicit DXGI attempts require the caller's original positive deadline.
        Finite native waits cannot bound an ininterruptible driver call.
        """
        if not self._ready:
            raise RuntimeError("Region capture is not inside its owned resource lifetime")
        integer(minimum_present_ticks, 0, 0x7fffffffffffffff, "source floor")
        if deadline_ticks is None:
            if self._storage.backend != "gdi":
                raise ValueError("DXGI capture requires an explicit original absolute deadline")
            deadline = 0x7fffffffffffffff
        else:
            deadline = integer(deadline_ticks, 1, 0x7fffffffffffffff, "capture deadline")
        native = self._native
        self.last_attempt = {"schema_version": 2, "backend": self._storage.backend, "accepted": False}
        begin = native.ticks()
        self.last_attempt["begin_ticks"] = begin
        geometry = self._verify()
        ownership_end = native.ticks()
        storage_begin = native.ticks()
        if storage_begin >= deadline:
            raise RuntimeError("Owned capture exceeded original absolute deadline before copy")
        primary = result = None
        try:
            result = self._storage.attempt(minimum_present_ticks, deadline)
            self.last_attempt["backend_evidence"] = asdict(result.metadata)
            if type(result.bgra) is bytes:
                self.last_attempt["byte_count"] = len(result.bgra)
                self.last_attempt["bgra"] = result.bgra[:1024].hex()
        except BaseException as error:
            primary = error
            self.last_attempt["backend_evidence"] = getattr(self._storage, "last_attempt", None)
            self.last_attempt["failure"] = str(primary)
        failures = [primary] if primary is not None else []
        storage_end = end = None
        for name, operation in (("storage_end_ticks", native.ticks), ("final_ownership", self._verify), ("end_ticks", native.ticks)):
            try:
                value = operation()
                if name == "storage_end_ticks":
                    storage_end = value
                elif name == "end_ticks":
                    end = value
                self.last_attempt[name] = asdict(value) if isinstance(value, Geometry) else value
            except BaseException as failure:
                failures.append(failure)
        if failures:
            if len(failures) == 1:
                raise failures[0]
            raise BaseExceptionGroup("Capture copy, clocks and final ownership failed", failures)
        fields = dict(hwnd=self.hwnd, pid=self.pid, creation_ticks=self.creation_ticks, geometry=geometry,
                      region=self.region, qpc_frequency=self._frequency, begin_ticks=begin,
                      ownership_before_end_ticks=ownership_end, storage_begin_ticks=storage_begin,
                      storage_end_ticks=storage_end, end_ticks=end, metadata=result.metadata, backend=result.backend)
        observed = RegionPending(**fields) if result.bgra is None else RegionSample(bgra=result.bgra, **fields)
        self.last_attempt = asdict(observed)
        if result.bgra is not None:
            self.last_attempt["bgra"] = result.bgra.hex()
        self.last_attempt["accepted"] = False
        validate_record(observed, frequency=self._frequency, deadline=deadline, minimum_present=minimum_present_ticks)
        self.last_attempt = observed
        return observed

    def close(self):
        """Release storage/process, then restore DPI; never discard pending resources."""
        self._ready = False
        if self._thread is not None and self._native.thread_id() != self._thread:
            raise RuntimeError("Region capture cleanup crossed its owning native thread")
        failures = []
        try:
            self._storage.close()
        except ExceptionGroup as error:
            failures.extend(error.exceptions)
        except Exception as error:
            failures.append(error)
        if self._process is not None:
            try:
                self._native.close_process(self._process)
                self._process = None
            except Exception as error:
                failures.append(error)
        if self._previous_dpi is not None and not self._storage.active and self._process is None:
            try:
                self._native.set_dpi_context(self._previous_dpi)
                if not self._native.dpi_equal(self._native.dpi_context(), self._previous_dpi):
                    raise RuntimeError("Native thread DPI context restoration could not be verified")
                self._previous_dpi = None
            except Exception as error:
                failures.append(error)
        if not self._storage.active and self._process is None and self._previous_dpi is None:
            _CAPTURE_OWNERS.pop(id(self), None)
        if failures:
            raise ExceptionGroup("Region capture resource cleanup failed", failures)

    def __exit__(self, kind, error, traceback):
        try:
            self.close()
        except BaseException as cleanup:
            if error is not None:
                raise BaseExceptionGroup("Region capture observation and cleanup failed", [error, cleanup])
            raise
        return False


class _Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_int32), ("y", ctypes.c_int32)]


class _Rect(ctypes.Structure):
    _fields_ = [(name, ctypes.c_int32) for name in ("left", "top", "right", "bottom")]


class _FileTime(ctypes.Structure):
    _fields_ = [("low", ctypes.c_uint32), ("high", ctypes.c_uint32)]


class _Win32Boundary:
    """Keep pointer-width FFI, resource calls and bounded z-order inventory private."""

    def __init__(self) -> None:
        if os.name != "nt":
            raise OSError("Actual region capture requires Windows")
        self._user = ctypes.WinDLL("user32", use_last_error=True)
        self._kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        handle, uint, boolean = ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int32
        pointer = ctypes.POINTER
        declarations = [
            (self._kernel, "OpenProcess", [uint, boolean, uint], handle),
            (self._kernel, "CloseHandle", [handle], boolean),
            (self._kernel, "WaitForSingleObject", [handle, uint], uint),
            (self._kernel, "GetProcessTimes", [handle] + [pointer(_FileTime)] * 4, boolean),
            (self._kernel, "GetCurrentThreadId", [], uint),
            (self._kernel, "QueryPerformanceCounter", [pointer(ctypes.c_int64)], boolean),
            (self._kernel, "QueryPerformanceFrequency", [pointer(ctypes.c_int64)], boolean),
            (self._user, "GetWindowThreadProcessId", [handle, pointer(uint)], uint),
            (self._user, "SetThreadDpiAwarenessContext", [handle], handle),
            (self._user, "GetThreadDpiAwarenessContext", [], handle),
            (self._user, "AreDpiAwarenessContextsEqual", [handle, handle], boolean),
            (self._user, "IsWindowVisible", [handle], boolean),
            (self._user, "IsIconic", [handle], boolean),
            (self._user, "GetAncestor", [handle, uint], handle),
            (self._user, "GetForegroundWindow", [], handle),
            (self._user, "GetClientRect", [handle, pointer(_Rect)], boolean),
            (self._user, "ClientToScreen", [handle, pointer(_Point)], boolean),
            (self._user, "GetDpiForWindow", [handle], uint),
            (self._user, "WindowFromPoint", [_Point], handle),
            (self._user, "GetWindow", [handle, uint], handle),
            (self._user, "GetWindowRect", [handle, pointer(_Rect)], boolean),
        ]
        for library, name, arguments, result in declarations:
            function = getattr(library, name)
            function.argtypes, function.restype = arguments, result

    @staticmethod
    def _checked(value: Any, operation: str) -> Any:
        if not value:
            raise OSError(ctypes.get_last_error(), f"{operation} failed")
        return value

    def thread_id(self) -> int:
        return self._checked(self._kernel.GetCurrentThreadId(), "GetCurrentThreadId")

    def set_dpi_context(self, context: int) -> int:
        return self._checked(self._user.SetThreadDpiAwarenessContext(context), "SetThreadDpiAwarenessContext")

    def dpi_context(self) -> int:
        return self._checked(self._user.GetThreadDpiAwarenessContext(), "GetThreadDpiAwarenessContext")

    def dpi_equal(self, left: int, right: int) -> bool:
        # Native contexts carry informational flags, so pointer equality is insufficient.
        return bool(self._user.AreDpiAwarenessContextsEqual(left, right))

    def frequency(self) -> int:
        value = ctypes.c_int64()
        self._checked(self._kernel.QueryPerformanceFrequency(ctypes.byref(value)), "QueryPerformanceFrequency")
        return value.value

    def ticks(self) -> int:
        value = ctypes.c_int64()
        self._checked(self._kernel.QueryPerformanceCounter(ctypes.byref(value)), "QueryPerformanceCounter")
        return value.value

    def open_process(self, pid: int) -> int:
        return self._checked(self._kernel.OpenProcess(0x1000 | 0x100000, False, pid), "OpenProcess")

    def process_alive(self, handle: int) -> bool:
        outcome = self._kernel.WaitForSingleObject(handle, 0)
        if outcome not in (0, 0x102):
            raise OSError(ctypes.get_last_error(), "Retained process wait failed")
        return outcome == 0x102

    def creation(self, handle: int) -> int:
        values = [_FileTime() for _ in range(4)]
        self._checked(self._kernel.GetProcessTimes(handle, *(ctypes.byref(value) for value in values)), "GetProcessTimes")
        return values[0].low | (values[0].high << 32)

    def window_pid(self, hwnd: int) -> int:
        pid = ctypes.c_uint32()
        self._checked(self._user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid)), "GetWindowThreadProcessId")
        return pid.value

    def root(self, hwnd: int) -> int:
        return self._checked(self._user.GetAncestor(hwnd, 2), "GetAncestor")

    def foreground(self) -> int:
        return self._user.GetForegroundWindow()

    def visible(self, hwnd: int) -> bool:
        return bool(self._user.IsWindowVisible(hwnd))

    def minimized(self, hwnd: int) -> bool:
        return bool(self._user.IsIconic(hwnd))

    def geometry(self, hwnd: int) -> Geometry:
        rectangle, origin = _Rect(), _Point()
        self._checked(self._user.GetClientRect(hwnd, ctypes.byref(rectangle)), "GetClientRect")
        self._checked(self._user.ClientToScreen(hwnd, ctypes.byref(origin)), "ClientToScreen")
        dpi = self._checked(self._user.GetDpiForWindow(hwnd), "GetDpiForWindow")
        return Geometry(rectangle.right - rectangle.left, rectangle.bottom - rectangle.top, dpi, origin.x, origin.y)

    def above(self, hwnd: int) -> list[int]:
        windows: list[int] = []
        current = hwnd
        for _ in range(256):
            ctypes.set_last_error(0)
            current = self._user.GetWindow(current, 3)
            if not current:
                if ctypes.get_last_error():
                    raise OSError(ctypes.get_last_error(), "GetWindow failed")
                return windows
            if current == hwnd or current in windows:
                raise RuntimeError("Native z-order inventory contains a cycle")
            windows.append(current)
        raise RuntimeError("Native z-order inventory exceeded its bounded capacity")

    def bounds(self, hwnd: int) -> tuple[int, int, int, int]:
        rectangle = _Rect()
        self._checked(self._user.GetWindowRect(hwnd, ctypes.byref(rectangle)), "GetWindowRect")
        return rectangle.left, rectangle.top, rectangle.right, rectangle.bottom

    def hit(self, x: int, y: int) -> int:
        return self._user.WindowFromPoint(_Point(x, y))

    def close_process(self, handle: int) -> None:
        self._checked(self._kernel.CloseHandle(handle), "CloseHandle")
