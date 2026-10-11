# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Observe bounded screen pixels from an owned exposed Windows client.

The returned timing brackets include observation work. They are not native
publication times and must not be adjusted by subtracting average capture cost.
The adapter writes no files, changes no windows and performs no input injection.
"""

from __future__ import annotations

import ctypes
import os
from dataclasses import dataclass
from typing import Any

_PER_MONITOR_AWARE_V2 = -4


def _integer(value: int, minimum: int, maximum: int, name: str) -> None:
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"{name} must be an integer in [{minimum}, {maximum}]")


@dataclass(frozen=True)
class Region:
    """Describe a 2..16-pixel rectangle in physical client coordinates."""

    x: int
    y: int
    width: int
    height: int

    def __post_init__(self) -> None:
        _integer(self.x, 0, 0x7fffffff, "region x")
        _integer(self.y, 0, 0x7fffffff, "region y")
        _integer(self.width, 2, 16, "region width")
        _integer(self.height, 2, 16, "region height")


@dataclass(frozen=True)
class Geometry:
    """Retain device client dimensions, DPI and physical screen origin."""

    width: int
    height: int
    dpi: int
    screen_x: int
    screen_y: int


@dataclass(frozen=True)
class RegionSample:
    """Retain one immutable top-down BGRA8 observation; alpha is unspecified.

    Tick fields use the recorded QPC frequency. Ownership checks are outside the
    copy bracket but inside the complete sample bracket. `end_ticks` is the
    conservative observed end; no timestamp claims the exact display transition.
    """

    bgra: bytes
    hwnd: int
    pid: int
    creation_ticks: int
    geometry: Geometry
    region: Region
    qpc_frequency: int
    begin_ticks: int
    ownership_before_end_ticks: int
    flush_begin_ticks: int | None
    flush_end_ticks: int | None
    copy_begin_ticks: int
    copy_end_ticks: int
    read_end_ticks: int
    end_ticks: int


class RegionCapture:
    """Own one retained process identity and reusable, thread-affine capture DIB.

    `creation_ticks` must come from the caller's verified process creation receipt.
    An owned thread-local Per Monitor v2 DPI context makes coordinates physical;
    the caller's previous context is restored after native resource cleanup.
    The injectable boundary implements the private native methods below; portable
    boundary tests do not constitute actual Windows display qualification.
    """

    def __init__(self, hwnd: int, pid: int, region: Region, *, creation_ticks: int,
                 flush: bool = False, boundary: Any | None = None) -> None:
        _integer(hwnd, 1, (1 << (ctypes.sizeof(ctypes.c_void_p) * 8)) - 1, "HWND")
        _integer(pid, 1, 0xffffffff, "PID")
        _integer(creation_ticks, 1, 0xffffffffffffffff, "process creation ticks")
        if not isinstance(region, Region) or type(flush) is not bool:
            raise ValueError("Capture requires a validated Region and boolean flush option")
        self.hwnd, self.pid, self.region = hwnd, pid, region
        self.creation_ticks, self.flush = creation_ticks, flush
        self._native = boundary if boundary is not None else _Win32Boundary()
        self._process = self._screen = self._dc = self._bitmap = self._previous = None
        self._pixels = None
        self._geometry: Geometry | None = None
        self._frequency = 0
        self._started = self._ready = False
        self._thread = None
        self._previous_dpi = None

    def __enter__(self) -> RegionCapture:
        if self._started:
            raise RuntimeError("A region capture lifetime cannot be entered again")
        self._started = True
        try:
            self._thread = self._native.thread_id()
            previous = self._native.set_dpi_context(_PER_MONITOR_AWARE_V2)
            if not previous:
                raise RuntimeError("Native thread DPI context could not be established")
            self._previous_dpi = previous
            self._verify_dpi()
            self._frequency = self._native.frequency()
            _integer(self._frequency, 1, 0x7fffffffffffffff, "QPC frequency")
            self._process = self._native.open_process(self.pid)
            self._geometry = self._verify()
            self._screen = self._native.screen_dc()
            self._dc = self._native.memory_dc(self._screen)
            self._bitmap, self._pixels = self._native.dib(self._screen, self.region.width, self.region.height)
            if not self._bitmap or not self._pixels:
                raise RuntimeError("CreateDIBSection did not return owned bitmap storage")
            self._previous = self._native.select(self._dc, self._bitmap)
            self._ready = True
            return self
        except BaseException as primary:
            try:
                self.close()
            except BaseException as cleanup:
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

    def sample(self) -> RegionSample:
        """Read current screen pixels once, with checked ownership on both sides."""
        if not self._ready:
            raise RuntimeError("Region capture is not inside its owned resource lifetime")
        native = self._native
        begin = native.ticks()
        geometry = self._verify()
        ownership_end = native.ticks()
        flush_begin = flush_end = None
        if self.flush:
            flush_begin = native.ticks()
            native.dwm_flush()
            flush_end = native.ticks()
        copy_begin = native.ticks()
        native.copy(self._dc, self._screen, geometry.screen_x + self.region.x,
                    geometry.screen_y + self.region.y, self.region.width, self.region.height)
        # CreateDIBSection requires completed GDI drawing before direct pointer reads.
        native.gdi_flush()
        copy_end = native.ticks()
        pixels = native.read(self._pixels, self.region.width * self.region.height * 4)
        read_end = native.ticks()
        self._verify()
        end = native.ticks()
        brackets = [begin, ownership_end]
        if flush_begin is not None:
            brackets.extend([flush_begin, flush_end])
        brackets.extend([copy_begin, copy_end, read_end, end])
        if any(type(tick) is not int or tick < 0 for tick in brackets) or any(a > b for a, b in zip(brackets, brackets[1:])):
            raise RuntimeError("Capture timing brackets are invalid or reversed")
        if type(pixels) is not bytes or len(pixels) != self.region.width * self.region.height * 4:
            raise RuntimeError("Native DIB read did not return the exact bounded region bytes")
        return RegionSample(pixels, self.hwnd, self.pid, self.creation_ticks, geometry, self.region,
                            self._frequency, begin, ownership_end, flush_begin, flush_end,
                            copy_begin, copy_end, read_end, end)

    def close(self) -> None:
        """Release resources, then restore DPI; retain failures for same-thread retry.

        Live handles keep the physical DPI guard active after a cleanup failure.
        A failed context restoration remains owned even after handles are closed.
        """
        self._ready = False
        if self._thread is not None and self._native.thread_id() != self._thread:
            raise RuntimeError("Region capture cleanup crossed its owning native thread")
        failures: list[Exception] = []

        def release(attribute: str, operation: Any) -> None:
            value = getattr(self, attribute)
            if value is not None:
                try:
                    operation(value)
                    setattr(self, attribute, None)
                except Exception as error:
                    failures.append(error)

        if self._previous is not None and self._dc is not None:
            try:
                self._native.select(self._dc, self._previous)
                self._previous = None
            except Exception as error:
                failures.append(error)
        bitmap_attempted = self._previous is None
        if bitmap_attempted:
            release("_bitmap", self._native.delete_bitmap)
        release("_dc", self._native.delete_dc)
        if self._dc is None:
            self._previous = None
            if not bitmap_attempted:
                release("_bitmap", self._native.delete_bitmap)
        release("_screen", self._native.release_screen)
        release("_process", self._native.close_process)
        if self._bitmap is None:
            self._pixels = None
        if self._previous_dpi is not None and all(value is None for value in (self._bitmap, self._dc, self._screen, self._process)):
            try:
                self._native.set_dpi_context(self._previous_dpi)
                if not self._native.dpi_equal(self._native.dpi_context(), self._previous_dpi):
                    raise RuntimeError("Native thread DPI context restoration could not be verified")
                self._previous_dpi = None
            except Exception as error:
                failures.append(error)
        if failures:
            raise ExceptionGroup("Region capture resource cleanup failed", failures)

    def __exit__(self, kind: object, error: BaseException | None, traceback: object) -> bool:
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


class _BitmapHeader(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("width", ctypes.c_int32), ("height", ctypes.c_int32),
                ("planes", ctypes.c_uint16), ("bits", ctypes.c_uint16), ("compression", ctypes.c_uint32),
                ("image_size", ctypes.c_uint32), ("x", ctypes.c_int32), ("y", ctypes.c_int32),
                ("used", ctypes.c_uint32), ("important", ctypes.c_uint32)]


class _BitmapInfo(ctypes.Structure):
    _fields_ = [("header", _BitmapHeader), ("colors", ctypes.c_uint32 * 3)]


class _Win32Boundary:
    """Keep pointer-width FFI, resource calls and bounded z-order inventory private."""

    def __init__(self) -> None:
        if os.name != "nt":
            raise OSError("Actual region capture requires Windows")
        self._user = ctypes.WinDLL("user32", use_last_error=True)
        self._gdi = ctypes.WinDLL("gdi32", use_last_error=True)
        self._kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self._dwm = ctypes.WinDLL("dwmapi", use_last_error=True)
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
            (self._user, "GetDC", [handle], handle),
            (self._user, "ReleaseDC", [handle, handle], ctypes.c_int32),
            (self._gdi, "CreateCompatibleDC", [handle], handle),
            (self._gdi, "CreateDIBSection", [handle, pointer(_BitmapInfo), uint, pointer(handle), handle, uint], handle),
            (self._gdi, "SelectObject", [handle, handle], handle),
            (self._gdi, "DeleteObject", [handle], boolean),
            (self._gdi, "DeleteDC", [handle], boolean),
            (self._gdi, "BitBlt", [handle] + [ctypes.c_int32] * 4 + [handle] + [ctypes.c_int32] * 2 + [uint], boolean),
            (self._gdi, "GdiFlush", [], boolean),
            (self._dwm, "DwmFlush", [], ctypes.c_int32),
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

    def screen_dc(self) -> int:
        return self._checked(self._user.GetDC(None), "GetDC")

    def memory_dc(self, screen: int) -> int:
        return self._checked(self._gdi.CreateCompatibleDC(screen), "CreateCompatibleDC")

    def dib(self, screen: int, width: int, height: int) -> tuple[int, int | None]:
        info = _BitmapInfo(_BitmapHeader(40, width, -height, 1, 32, 0, width * height * 4, 0, 0, 0, 0))
        pixels = ctypes.c_void_p()
        bitmap = self._checked(self._gdi.CreateDIBSection(screen, ctypes.byref(info), 0, ctypes.byref(pixels), None, 0), "CreateDIBSection")
        # Transfer the bitmap even on an invalid pointer so its owner can retain
        # failed cleanup handles and attempt explicit teardown later.
        return bitmap, pixels.value

    def select(self, dc: int, bitmap: int) -> int:
        value = self._checked(self._gdi.SelectObject(dc, bitmap), "SelectObject")
        if value == ctypes.c_void_p(-1).value:
            raise OSError(ctypes.get_last_error(), "SelectObject returned HGDI_ERROR")
        return value

    def copy(self, dc: int, screen: int, x: int, y: int, width: int, height: int) -> None:
        self._checked(self._gdi.BitBlt(dc, 0, 0, width, height, screen, x, y, 0x00cc0020 | 0x40000000), "BitBlt")

    def gdi_flush(self) -> None:
        self._checked(self._gdi.GdiFlush(), "GdiFlush")

    def dwm_flush(self) -> None:
        result = self._dwm.DwmFlush()
        if result != 0:
            raise RuntimeError(f"DwmFlush failed with HRESULT {result:#x}")

    def read(self, pointer: int, size: int) -> bytes:
        return ctypes.string_at(pointer, size)

    def delete_bitmap(self, bitmap: int) -> None:
        self._checked(self._gdi.DeleteObject(bitmap), "DeleteObject")

    def delete_dc(self, dc: int) -> None:
        self._checked(self._gdi.DeleteDC(dc), "DeleteDC")

    def release_screen(self, dc: int) -> None:
        self._checked(self._user.ReleaseDC(None, dc), "ReleaseDC")

    def close_process(self, handle: int) -> None:
        self._checked(self._kernel.CloseHandle(handle), "CloseHandle")
