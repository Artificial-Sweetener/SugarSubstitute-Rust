# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Own the reusable GDI copy storage; window/security witnesses belong to RegionCapture."""
from __future__ import annotations
import ctypes
import os
from typing import Any
try:
    from .region_records import CopyResult, GdiMetadata
except ImportError:
    from region_records import CopyResult, GdiMetadata


class GdiCopyStorage:
    """Own screen/DC/DIB selection and preserve failed cleanup handles for retry."""
    backend = "gdi"

    def __init__(self, *, flush=False, boundary=None):
        self.native = boundary if boundary is not None else _GdiBoundary()
        self.flush = flush
        self._screen = self._dc = self._bitmap = self._previous = self._pixels = None
        self.geometry = self.region = self.clock = None
        self.last_attempt = None

    @property
    def active(self):
        return any(v is not None for v in (self._screen, self._dc, self._bitmap))

    def open(self, geometry, region, frequency, thread, clock):
        self.geometry, self.region, self.clock = geometry, region, clock
        self._screen = self.native.screen_dc()
        self._dc = self.native.memory_dc(self._screen)
        self._bitmap, self._pixels = self.native.dib(self._screen, region.width, region.height)
        if not self._bitmap or not self._pixels:
            raise RuntimeError("CreateDIBSection did not return owned bitmap storage")
        self._previous = self.native.select(self._dc, self._bitmap)

    def attempt(self, minimum_present, deadline):
        native, clock = self.native, self.clock
        self.last_attempt = {"backend": "gdi", "flush_requested": self.flush}
        flush_begin = flush_end = None
        if self.flush:
            flush_begin = clock()
            self.last_attempt["flush_begin_ticks"] = flush_begin
            native.dwm_flush()
            self.last_attempt["dwm_flush_completed"] = True
            flush_end = clock()
            self.last_attempt["flush_end_ticks"] = flush_end
        copy_begin = clock()
        self.last_attempt["copy_begin_ticks"] = copy_begin
        native.copy(self._dc, self._screen, self.geometry.screen_x + self.region.x,
                    self.geometry.screen_y + self.region.y, self.region.width, self.region.height)
        self.last_attempt["copy_completed"] = True
        # Complete GDI writes before reading CreateDIBSection storage directly.
        native.gdi_flush()
        self.last_attempt["gdi_flush_completed"] = True
        copy_end = clock()
        self.last_attempt["copy_end_ticks"] = copy_end
        pixels = native.read(self._pixels, self.region.width * self.region.height * 4)
        self.last_attempt["read_returned"] = True
        if type(pixels) is bytes:
            self.last_attempt.update(byte_count=len(pixels), bgra=pixels[:1024].hex())
        read_end = clock()
        self.last_attempt["read_end_ticks"] = read_end
        return CopyResult("gdi", pixels, GdiMetadata(flush_begin, flush_end, copy_begin, copy_end, read_end))

    def close(self):
        failures = []
        def release(attribute, operation):
            value = getattr(self, attribute)
            if value is not None:
                try:
                    operation(value)
                    setattr(self, attribute, None)
                except Exception as error:
                    failures.append(error)
        if self._previous is not None and self._dc is not None:
            try:
                self.native.select(self._dc, self._previous)
                self._previous = None
            except Exception as error:
                failures.append(error)
        bitmap_attempted = self._previous is None
        if bitmap_attempted:
            release("_bitmap", self.native.delete_bitmap)
        release("_dc", self.native.delete_dc)
        if self._dc is None:
            self._previous = None
            if not bitmap_attempted:
                release("_bitmap", self.native.delete_bitmap)
        release("_screen", self.native.release_screen)
        if self._bitmap is None:
            self._pixels = None
        if failures:
            raise ExceptionGroup("GDI storage cleanup failed", failures)


class _BitmapHeader(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("width", ctypes.c_int32), ("height", ctypes.c_int32),
                ("planes", ctypes.c_uint16), ("bits", ctypes.c_uint16), ("compression", ctypes.c_uint32),
                ("image_size", ctypes.c_uint32), ("x", ctypes.c_int32), ("y", ctypes.c_int32),
                ("used", ctypes.c_uint32), ("important", ctypes.c_uint32)]


class _BitmapInfo(ctypes.Structure):
    _fields_ = [("header", _BitmapHeader), ("colors", ctypes.c_uint32 * 3)]


class _GdiBoundary:
    """Keep all GDI allocation, copying and resource FFI with its lifetime owner."""
    def __init__(self):
        if os.name != "nt":
            raise OSError("Actual GDI storage requires Windows")
        self._user = ctypes.WinDLL("user32", use_last_error=True)
        self._gdi = ctypes.WinDLL("gdi32", use_last_error=True)
        self._dwm = ctypes.WinDLL("dwmapi", use_last_error=True)
        h, u, b, p = ctypes.c_void_p, ctypes.c_uint32, ctypes.c_int32, ctypes.POINTER
        declarations = [
            (self._user, "GetDC", [h], h), (self._user, "ReleaseDC", [h, h], b),
            (self._gdi, "CreateCompatibleDC", [h], h),
            (self._gdi, "CreateDIBSection", [h, p(_BitmapInfo), u, p(h), h, u], h),
            (self._gdi, "SelectObject", [h, h], h), (self._gdi, "DeleteObject", [h], b),
            (self._gdi, "DeleteDC", [h], b),
            (self._gdi, "BitBlt", [h] + [b] * 4 + [h] + [b] * 2 + [u], b),
            (self._gdi, "GdiFlush", [], b), (self._dwm, "DwmFlush", [], b)]
        for library, name, args, result in declarations:
            function = getattr(library, name)
            function.argtypes, function.restype = args, result

    @staticmethod
    def _checked(value: Any, operation: str) -> Any:
        if not value:
            raise OSError(ctypes.get_last_error(), f"{operation} failed")
        return value

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
