# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Qualify an owned Windows splash process, native material, input and CPU submissions."""

from __future__ import annotations

import argparse
import ctypes
import json
import math
import os
import subprocess
import sys
import time
from ctypes import wintypes
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from PIL import Image

try:
    from .native_decisions import accessible_controls, startup_decision
except ImportError:
    from native_decisions import accessible_controls, startup_decision

class BitmapHeader(ctypes.Structure):
    """Match the native top-down 32-bit BITMAPINFOHEADER ABI."""

    _fields_ = [
        ("size", wintypes.DWORD),
        ("width", wintypes.LONG),
        ("height", wintypes.LONG),
        ("planes", wintypes.WORD),
        ("bits", wintypes.WORD),
        ("compression", wintypes.DWORD),
        ("image_size", wintypes.DWORD),
        ("x", wintypes.LONG),
        ("y", wintypes.LONG),
        ("used", wintypes.DWORD),
        ("important", wintypes.DWORD),
    ]


class BitmapInfo(ctypes.Structure):
    """Provide storage for the native raster descriptor and unused RGB color table."""

    _fields_ = [("header", BitmapHeader), ("colors", wintypes.DWORD * 3)]


class MonitorInfo(ctypes.Structure):
    """Retain the native monitor rectangle and its taskbar-excluding work area."""

    _fields_ = [
        ("size", wintypes.DWORD),
        ("monitor", wintypes.RECT),
        ("work", wintypes.RECT),
        ("flags", wintypes.DWORD),
    ]


@dataclass(frozen=True)
class Client:
    """Retain device geometry and native DPI without assuming a workstation scale."""

    width: int
    height: int
    dpi: int

    @property
    def scale(self) -> float:
        """Convert logical presentation coordinates to native device pixels."""
        return self.dpi / 96


class MouseInput(ctypes.Structure):
    """Match MOUSEINPUT, including pointer-width extra information."""

    _fields_ = [("x", wintypes.LONG), ("y", wintypes.LONG), ("data", wintypes.DWORD),
                ("flags", wintypes.DWORD), ("time", wintypes.DWORD), ("extra", ctypes.c_size_t)]


class InputPayload(ctypes.Union):
    """Retain the largest INPUT union member used by injected mouse events."""

    _fields_ = [("mouse", MouseInput)]


class MouseEvent(ctypes.Structure):
    """Match INPUT for SendInput; mouse is the largest native union member."""

    _fields_ = [("kind", wintypes.DWORD), ("payload", InputPayload)]


class NativePointer:
    """Own one bounded mouse gesture and conditionally restore the user's untouched cursor."""

    def __init__(self, native: NativeWindows, hwnd: int, pid: int) -> None:
        """Borrow only the verified caption window and declare mouse ABI contracts."""
        self.native, self.hwnd, self.pid = native, hwnd, pid
        self.original: tuple[int, int] | None = None
        self.last: tuple[int, int] | None = None
        self.pressed = False
        for function, arguments, result in (
            (native.user.SendInput, [wintypes.UINT, ctypes.POINTER(MouseEvent), ctypes.c_int], wintypes.UINT),
            (native.user.GetSystemMetrics, [ctypes.c_int], ctypes.c_int),
            (native.user.GetAsyncKeyState, [ctypes.c_int], ctypes.c_short),
            (native.user.WindowFromPoint, [wintypes.POINT], wintypes.HWND),
            (native.user.GetAncestor, [wintypes.HWND, wintypes.UINT], wintypes.HWND),
            (native.user.GetForegroundWindow, [], wintypes.HWND),
        ):
            function.argtypes, function.restype = arguments, result

    def cursor(self) -> tuple[int, int]:
        """Read the actual screen cursor without moving it."""
        point = wintypes.POINT()
        if not self.native.user.GetCursorPos(ctypes.byref(point)):
            raise ctypes.WinError(ctypes.get_last_error())
        return point.x, point.y

    def __enter__(self) -> NativePointer:
        """Reject existing human button state and save the cursor before injecting anything."""
        self.native.client(self.hwnd, self.pid)
        if any(self.native.user.GetAsyncKeyState(key) & 0x8000 for key in (1, 2, 4, 5, 6)):
            raise RuntimeError("A mouse button is already held; owned pointer qualification rejected")
        self.original = self.cursor()
        self.last = self.original
        return self

    def unchanged_cursor(self) -> None:
        """Stop on human cursor movement rather than reclaiming the pointer."""
        current = self.cursor()
        if self.last is None or max(abs(a - b) for a, b in zip(current, self.last)) > 1:
            raise RuntimeError("The cursor moved outside the owned gesture; human position is preserved")

    def send(self, point: tuple[int, int], flags: int) -> None:
        """Inject one event over signed virtual-desktop coordinates without acceleration."""
        left, top, width, height = (self.native.user.GetSystemMetrics(index) for index in (76, 77, 78, 79))
        if width <= 1 or height <= 1 or not (left <= point[0] < left + width and top <= point[1] < top + height):
            raise RuntimeError("Owned pointer target is outside the real virtual desktop")
        x = math.floor((point[0] - left) * 65535 / (width - 1) + .5)
        y = math.floor((point[1] - top) * 65535 / (height - 1) + .5)
        event = MouseEvent(0, InputPayload(MouseInput(x, y, 0, flags | 0xC001 | 0x2000, 0, 0)))
        if self.native.user.SendInput(1, ctypes.byref(event), ctypes.sizeof(event)) != 1:
            raise RuntimeError("SendInput did not insert the owned mouse event")

    def owned_hit(self, point: tuple[int, int]) -> None:
        """Reject foreign foreground or hit-test windows before pressing caption controls."""
        self.native.client(self.hwnd, self.pid)
        hit = self.native.user.WindowFromPoint(wintypes.POINT(*point))
        if not hit or self.native.user.GetAncestor(hit, 2) != self.hwnd:
            raise RuntimeError("The physical pointer target is not the owned window")
        if self.native.user.GetAncestor(self.native.user.GetForegroundWindow(), 2) != self.hwnd:
            raise RuntimeError("The owned pointer window is not foreground")

    def move_screen(self, point: tuple[int, int]) -> None:
        """Move via SendInput, wait for actual cursor placement, then verify the owned hit."""
        self.unchanged_cursor()
        self.owned_hit(point)
        self.send(point, 0)
        deadline = time.perf_counter() + .5
        while max(abs(a - b) for a, b in zip(self.cursor(), point)) > 1:
            if time.perf_counter() >= deadline:
                raise RuntimeError("The owned injected pointer did not reach its screen target")
            time.sleep(.005)
        self.last = self.cursor()
        self.owned_hit(self.last)

    def move(self, logical: tuple[float, float]) -> None:
        """Convert current logical client coordinates to actual screen coordinates."""
        client = self.native.client(self.hwnd, self.pid)
        point = wintypes.POINT(round(logical[0] * client.scale), round(logical[1] * client.scale))
        if not (0 <= point.x < client.width and 0 <= point.y < client.height):
            raise RuntimeError("Owned pointer logical target is outside the client")
        if not self.native.user.ClientToScreen(self.hwnd, ctypes.byref(point)):
            raise ctypes.WinError(ctypes.get_last_error())
        self.move_screen((point.x, point.y))

    def press(self) -> None:
        """Insert an actual left-button down at a currently verified owned client point."""
        self.unchanged_cursor()
        self.owned_hit(self.cursor())
        if self.pressed or self.native.user.GetAsyncKeyState(1) & 0x8000:
            raise RuntimeError("A left button is already held before the owned press")
        self.pressed = True
        self.send(self.cursor(), 2)

    def release(self) -> None:
        """Release the harness button at its verified current owned point."""
        self.unchanged_cursor()
        self.owned_hit(self.cursor())
        if not self.pressed:
            raise RuntimeError("No owned mouse press exists to release")
        self.send(self.cursor(), 4)
        self.pressed = False

    def __exit__(self, kind: object, error: BaseException | None, traceback: object) -> bool:
        """Release injected button state even after ownership loss; never overwrite human movement."""
        failures: list[Exception] = []
        if self.pressed:
            try:
                self.send(self.cursor(), 4)
                self.pressed = False
            except Exception as cleanup:
                failures.append(cleanup)
        try:
            self.unchanged_cursor()
            if self.original is not None:
                self.send(self.original, 0)
        except Exception as cleanup:
            failures.append(cleanup)
        if failures:
            if isinstance(error, Exception):
                failures.insert(0, error)
            raise ExceptionGroup("Owned pointer gesture and cleanup failures", failures)
        return False


class NativeWindows:
    """Own Win32 declarations and borrow handles only from the qualified process."""

    def __init__(self) -> None:
        """Declare pointer-width APIs before obtaining any native handles."""
        self.user = ctypes.WinDLL("user32", use_last_error=True)
        self.gdi = ctypes.WinDLL("gdi32", use_last_error=True)
        self.dwm = ctypes.WinDLL("dwmapi", use_last_error=True)
        self.callback_type = ctypes.WINFUNCTYPE(
            wintypes.BOOL, wintypes.HWND, wintypes.LPARAM
        )
        declarations: list[tuple[Any, list[Any], Any]] = [
            (self.user.SetProcessDpiAwarenessContext, [ctypes.c_void_p], wintypes.BOOL),
            (self.user.GetCursorPos, [ctypes.POINTER(wintypes.POINT)], wintypes.BOOL),
            (
                self.user.MonitorFromPoint,
                [wintypes.POINT, wintypes.DWORD],
                wintypes.HANDLE,
            ),
            (
                self.user.GetMonitorInfoW,
                [wintypes.HANDLE, ctypes.POINTER(MonitorInfo)],
                wintypes.BOOL,
            ),
            (
                self.user.EnumWindows,
                [self.callback_type, wintypes.LPARAM],
                wintypes.BOOL,
            ),
            (
                self.user.GetWindowThreadProcessId,
                [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)],
                wintypes.DWORD,
            ),
            (self.user.IsWindowVisible, [wintypes.HWND], wintypes.BOOL),
            (
                self.user.GetClientRect,
                [wintypes.HWND, ctypes.POINTER(wintypes.RECT)],
                wintypes.BOOL,
            ),
            (
                self.user.ClientToScreen,
                [wintypes.HWND, ctypes.POINTER(wintypes.POINT)],
                wintypes.BOOL,
            ),
            (self.user.GetDpiForWindow, [wintypes.HWND], wintypes.UINT),
            (
                self.user.GetWindowRect,
                [wintypes.HWND, ctypes.POINTER(wintypes.RECT)],
                wintypes.BOOL,
            ),
            (self.user.GetWindow, [wintypes.HWND, wintypes.UINT], wintypes.HWND),
            (self.user.IsIconic, [wintypes.HWND], wintypes.BOOL),
            (self.user.GetDC, [wintypes.HWND], wintypes.HDC),
            (self.user.ReleaseDC, [wintypes.HWND, wintypes.HDC], ctypes.c_int),
            (
                self.user.PrintWindow,
                [wintypes.HWND, wintypes.HDC, wintypes.UINT],
                wintypes.BOOL,
            ),
            (
                self.user.PostMessageW,
                [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM],
                wintypes.BOOL,
            ),
            (self.gdi.CreateCompatibleDC, [wintypes.HDC], wintypes.HDC),
            (
                self.gdi.CreateCompatibleBitmap,
                [wintypes.HDC, ctypes.c_int, ctypes.c_int],
                wintypes.HBITMAP,
            ),
            (self.gdi.SelectObject, [wintypes.HDC, wintypes.HGDIOBJ], wintypes.HGDIOBJ),
            (self.gdi.DeleteObject, [wintypes.HGDIOBJ], wintypes.BOOL),
            (self.gdi.DeleteDC, [wintypes.HDC], wintypes.BOOL),
            (
                self.gdi.BitBlt,
                [
                    wintypes.HDC,
                    ctypes.c_int,
                    ctypes.c_int,
                    ctypes.c_int,
                    ctypes.c_int,
                    wintypes.HDC,
                    ctypes.c_int,
                    ctypes.c_int,
                    wintypes.DWORD,
                ],
                wintypes.BOOL,
            ),
            (
                self.gdi.GetDIBits,
                [
                    wintypes.HDC,
                    wintypes.HBITMAP,
                    wintypes.UINT,
                    wintypes.UINT,
                    ctypes.c_void_p,
                    ctypes.POINTER(BitmapInfo),
                    wintypes.UINT,
                ],
                ctypes.c_int,
            ),
            (
                self.dwm.DwmGetWindowAttribute,
                [wintypes.HWND, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD],
                ctypes.c_long,
            ),
        ]
        for function, arguments, result in declarations:
            function.argtypes = arguments
            function.restype = result
        if not self.user.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4)):
            raise ctypes.WinError(ctypes.get_last_error())

    def owns(self, hwnd: int, pid: int) -> bool:
        """Reject destroyed, reused or unrelated native handles before every operation."""
        actual = wintypes.DWORD()
        self.user.GetWindowThreadProcessId(hwnd, ctypes.byref(actual))
        return actual.value == pid

    def cursor_work_area(self) -> tuple[int, int, int, int]:
        """Observe cursor monitor placement without moving the user's cursor."""
        cursor = wintypes.POINT()
        info = MonitorInfo()
        info.size = ctypes.sizeof(info)
        if not self.user.GetCursorPos(ctypes.byref(cursor)):
            raise ctypes.WinError(ctypes.get_last_error())
        monitor = self.user.MonitorFromPoint(cursor, 1)
        if not self.user.GetMonitorInfoW(monitor, ctypes.byref(info)):
            raise ctypes.WinError(ctypes.get_last_error())
        return info.work.left, info.work.top, info.work.right, info.work.bottom

    def placement(
        self, hwnd: int, pid: int, work: tuple[int, int, int, int]
    ) -> dict[str, object]:
        """Verify owned physical bounds against the launch cursor's stable monitor work area."""
        self.client(hwnd, pid)
        rect = wintypes.RECT()
        if not self.user.GetWindowRect(hwnd, ctypes.byref(rect)):
            raise ctypes.WinError(ctypes.get_last_error())
        expected = [
            work[0] + (work[2] - work[0] - (rect.right - rect.left)) // 2,
            work[1] + (work[3] - work[1] - (rect.bottom - rect.top)) // 2,
        ]
        observed = [rect.left, rect.top]
        stable = self.cursor_work_area() == work
        centered = observed == expected if stable else None
        if centered is False:
            raise RuntimeError(
                f"Owned native origin {observed} does not match cursor work-area center {expected}"
            )
        return {
            "cursor_work_area": work,
            "cursor_monitor_stable": stable,
            "expected_origin": expected,
            "observed_origin": observed,
            "centered": centered,
        }

    def find_window(self, process: subprocess.Popen[bytes], deadline: float) -> int:
        """Wait for an observable owned visible client, excluding Tao helper windows."""
        while time.perf_counter() < deadline and process.poll() is None:
            candidates: list[int] = []

            def inspect(hwnd: int, _: int, collection: list[int] = candidates) -> bool:
                """Retain only the launched process's nonempty visible native client."""
                rect = wintypes.RECT()
                if (
                    self.owns(hwnd, process.pid)
                    and self.user.IsWindowVisible(hwnd)
                    and self.user.GetClientRect(hwnd, ctypes.byref(rect))
                    and rect.right >= 100
                    and rect.bottom >= 100
                ):
                    collection.append(hwnd)
                return True

            callback = self.callback_type(inspect)
            if not self.user.EnumWindows(callback, 0):
                raise ctypes.WinError(ctypes.get_last_error())
            if candidates:
                return candidates[0]
            time.sleep(0.005)
        raise RuntimeError(
            "The owned process did not expose a visible client within the deadline"
        )

    def client(self, hwnd: int, pid: int) -> Client:
        """Read physical client geometry from the verified owned native window."""
        if not self.owns(hwnd, pid):
            raise RuntimeError(
                "The qualified window was destroyed or changed ownership"
            )
        rect = wintypes.RECT()
        if not self.user.GetClientRect(hwnd, ctypes.byref(rect)):
            raise ctypes.WinError(ctypes.get_last_error())
        dpi = self.user.GetDpiForWindow(hwnd)
        if rect.right <= 0 or rect.bottom <= 0 or dpi == 0:
            raise RuntimeError("The owned client has invalid dimensions or DPI")
        return Client(rect.right, rect.bottom, dpi)

    def attributes(self, hwnd: int, pid: int) -> dict[str, dict[str, int]]:
        """Record DWM's accepted backdrop, theme and rounded-corner attributes."""
        self.client(hwnd, pid)
        result: dict[str, dict[str, int]] = {}
        for name, attribute in (("backdrop", 38), ("dark", 20), ("corner", 33)):
            value = ctypes.c_int()
            status = self.dwm.DwmGetWindowAttribute(
                hwnd, attribute, ctypes.byref(value), 4
            )
            result[name] = {"hresult": status, "value": value.value}
        return result

    def occluded(self, hwnd: int, pid: int) -> bool:
        """Conservatively reject any other visible window above the owned client rectangle."""
        client = self.client(hwnd, pid)
        origin = wintypes.POINT()
        if not self.user.ClientToScreen(hwnd, ctypes.byref(origin)):
            raise ctypes.WinError(ctypes.get_last_error())
        visited: set[int] = set()
        previous = self.user.GetWindow(hwnd, 3)
        while previous and previous not in visited:
            visited.add(previous)
            rect = wintypes.RECT()
            if (
                self.user.IsWindowVisible(previous)
                and not self.user.IsIconic(previous)
                and self.user.GetWindowRect(previous, ctypes.byref(rect))
                and rect.left < origin.x + client.width
                and rect.right > origin.x
                and rect.top < origin.y + client.height
                and rect.bottom > origin.y
            ):
                return True
            previous = self.user.GetWindow(previous, 3)
        return False

    def capture(self, hwnd: int, pid: int, *, compositor: bool = False) -> Image.Image:
        """Capture the owned client; compositor capture requires a fully unoccluded rectangle."""
        client = self.client(hwnd, pid)
        if compositor and self.occluded(hwnd, pid):
            raise RuntimeError(
                "The owned client is occluded; compositor capture is unavailable"
            )
        source = self.user.GetDC(None if compositor else hwnd)
        dc = self.gdi.CreateCompatibleDC(source)
        bitmap = self.gdi.CreateCompatibleBitmap(source, client.width, client.height)
        if not source or not dc or not bitmap:
            if bitmap:
                self.gdi.DeleteObject(bitmap)
            if dc:
                self.gdi.DeleteDC(dc)
            if source:
                self.user.ReleaseDC(None if compositor else hwnd, source)
            raise ctypes.WinError(ctypes.get_last_error())
        previous = self.gdi.SelectObject(dc, bitmap)
        try:
            if previous in (None, 0, ctypes.c_void_p(-1).value):
                previous = None
                raise RuntimeError(
                    "SelectObject failed to select the owned capture bitmap"
                )
            if compositor:
                origin = wintypes.POINT()
                if not self.user.ClientToScreen(hwnd, ctypes.byref(origin)):
                    raise ctypes.WinError(ctypes.get_last_error())
                if self.dwm.DwmFlush() != 0:
                    raise RuntimeError(
                        "DWM did not complete the owned compositor capture flush"
                    )
                if not self.gdi.BitBlt(
                    dc,
                    0,
                    0,
                    client.width,
                    client.height,
                    source,
                    origin.x,
                    origin.y,
                    0x00CC0020 | 0x40000000,
                ):
                    raise ctypes.WinError(ctypes.get_last_error())
            elif not self.user.PrintWindow(hwnd, dc, 3):
                raise ctypes.WinError(ctypes.get_last_error())
            restored = self.gdi.SelectObject(dc, previous)
            if restored in (None, 0, ctypes.c_void_p(-1).value):
                raise RuntimeError(
                    "SelectObject failed to restore the capture DC selection"
                )
            previous = None
            info = BitmapInfo(
                BitmapHeader(
                    ctypes.sizeof(BitmapHeader),
                    client.width,
                    -client.height,
                    1,
                    32,
                    0,
                    client.width * client.height * 4,
                    0,
                    0,
                    0,
                    0,
                )
            )
            storage = (ctypes.c_ubyte * (client.width * client.height * 4))()
            if (
                self.gdi.GetDIBits(
                    dc, bitmap, 0, client.height, storage, ctypes.byref(info), 0
                )
                != client.height
            ):
                raise ctypes.WinError(ctypes.get_last_error())
            return Image.frombytes(
                "RGB", (client.width, client.height), bytes(storage), "raw", "BGRX"
            )
        finally:
            failures: list[str] = []
            if previous:
                restored = self.gdi.SelectObject(dc, previous)
                if restored in (None, 0, ctypes.c_void_p(-1).value):
                    failures.append("restore capture DC selection")
            if not self.gdi.DeleteObject(bitmap):
                failures.append("delete capture bitmap")
            if not self.gdi.DeleteDC(dc):
                failures.append("delete capture DC")
            if not self.user.ReleaseDC(None if compositor else hwnd, source):
                failures.append("release borrowed capture DC")
            if failures:
                raise RuntimeError(
                    "Native capture cleanup failed: " + ", ".join(failures)
                )

    def post(self, hwnd: int, pid: int, message: int, value: int, detail: int) -> None:
        """Send qualification input exclusively to the verified owned native window."""
        self.client(hwnd, pid)
        if not self.user.PostMessageW(hwnd, message, value, detail):
            raise ctypes.WinError(ctypes.get_last_error())


def has_mascot(image: Image.Image, scale: float) -> bool:
    """Recognize nonuniform saturated character paint without assuming exact font or DPI pixels."""
    crop = image.crop(
        (round(83 * scale), round(7 * scale), round(470 * scale), round(393 * scale))
    )
    raw = crop.tobytes()
    pixels = zip(raw[0::3], raw[1::3], raw[2::3])
    saturated = [
        pixel
        for pixel in pixels
        if pixel[0] > 160 and pixel[0] - pixel[1] > 40 and pixel[0] - pixel[2] > 10
    ]
    return len(saturated) > 200 and len(set(saturated)) > 64


def write_json(path: Path, value: object) -> None:
    """Save an owned qualification receipt with complete UTF-8 JSON."""
    path.write_text(
        json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def rgb_pixel(image: Image.Image, point: tuple[int, int]) -> tuple[int, int, int]:
    """Require the three-channel native RGB capture before comparing view state."""
    pixel = image.getpixel(point)
    if not isinstance(pixel, tuple) or len(pixel) != 3:
        raise ValueError("Native capture does not contain three-channel RGB pixels")
    return pixel[0], pixel[1], pixel[2]


def accessibility_snapshot(native: NativeWindows, hwnd: int, pid: int, *, action: str = "Inspect",
                           name: str = "") -> list[dict[str, object]]:
    """Recheck native ownership before a bounded public UI Automation query or action."""
    native.client(hwnd, pid)
    command = [str(Path(os.environ["SystemRoot"]) / "System32" / "WindowsPowerShell" / "v1.0" / "powershell.exe"),
               "-NoProfile", "-File", str(Path(__file__).with_name("accessibility_windows.ps1")),
               "-WindowHandle", str(hwnd), "-ProcessId", str(pid), "-Action", action]
    if name:
        command.extend(["-ControlName", name])
    snapshot = subprocess.run(command, check=False, capture_output=True, timeout=5)
    if snapshot.returncode != 0:
        raise RuntimeError(
            f"Owned UI Automation {action} failed with status {snapshot.returncode}: "
            + snapshot.stderr.decode("utf-8-sig", errors="replace")[:4096]
        )
    return json.loads(snapshot.stdout.decode("utf-8-sig"))


def observe_disclosure(native: NativeWindows, hwnd: int, process: subprocess.Popen[bytes],
                       baseline: tuple[int, int, int], scale: float, deadline: float) -> Image.Image:
    """Require a real visible console change after an accessible or keyboard action."""
    while time.perf_counter() < deadline and process.poll() is None:
        image = native.capture(hwnd, process.pid)
        after = rgb_pixel(image, (round(30 * scale), round(400 * scale)))
        if max(abs(left - right) for left, right in zip(baseline, after)) > 5:
            return image
        time.sleep(0.005)
    raise RuntimeError("Accessible caption action did not change the displayed disclosure state")


def observe_compositor(native: NativeWindows, hwnd: int, process: subprocess.Popen[bytes],
                       deadline: float) -> tuple[Image.Image, float]:
    """Wait for owned, unoccluded character paint, returning the external observation time."""
    while time.perf_counter() < deadline and process.poll() is None:
        native.client(hwnd, process.pid)
        image = native.capture(hwnd, process.pid, compositor=True)
        observed = time.perf_counter()
        client = native.client(hwnd, process.pid)
        if native.occluded(hwnd, process.pid):
            raise RuntimeError("The owned client became occluded during compositor observation")
        if not isinstance(image, Image.Image) or image.size != (client.width, client.height):
            raise RuntimeError("Compositor observation did not return the complete owned client frame")
        if observed < deadline and process.poll() is None and has_mascot(image, client.scale):
            return image, observed
        time.sleep(0.005)
    raise RuntimeError("The owned compositor did not expose character paint within the deadline")


def observe_caption(native: NativeWindows, hwnd: int, process: subprocess.Popen[bytes],
                    baseline: bytes, bounds: tuple[int, int, int, int], *, restored: bool) -> None:
    """Observe pressed feedback and neutral restoration without requiring animated content."""
    deadline = time.perf_counter() + 1
    while time.perf_counter() < deadline and process.poll() is None:
        current = native.capture(hwnd, process.pid).crop(bounds).tobytes()
        scale = (bounds[2] - bounds[0]) / 46
        point = (round(6 * scale), round(8 * scale))
        offset = (point[1] * (bounds[2] - bounds[0]) + point[0]) * 3
        pressed = tuple(current[offset:offset + 3])
        pressed_feedback = max(abs(left - right) for left, right in zip(pressed, (241, 112, 122))) <= 2
        observed = current == baseline if restored else pressed_feedback
        if observed:
            return
        time.sleep(0.005)
    raise RuntimeError("The native close caption did not restore neutral paint" if restored
                       else "The native close caption did not display pressed paint")


def qualify(arguments: argparse.Namespace) -> None:
    """Run one bounded native proof, save distinct receipts, and preserve process failure output."""
    native = NativeWindows()
    output: Path = arguments.output
    output.mkdir(parents=True, exist_ok=True)
    submission = output / "submission.json"
    if submission.exists():
        raise RuntimeError(
            "Choose a fresh output directory; existing qualification receipts are preserved"
        )
    cursor_work = native.cursor_work_area()
    started = time.perf_counter()
    command = [str(arguments.executable), "--duration-ms", str(arguments.duration_ms),
               "--receipt", str(submission), "--locale", arguments.locale]
    if arguments.light:
        command.append("--light")
    if arguments.reduced_motion:
        command.append("--reduced-motion")
    process = subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        deadline = started + max(0.5, arguments.duration_ms / 1000 - 0.25)
        hwnd = native.find_window(process, deadline)
        client = native.client(hwnd, process.pid)
        attributes = native.attributes(hwnd, process.pid)
        while True:
            image = native.capture(hwnd, process.pid)
            if has_mascot(image, client.scale):
                break
            if time.perf_counter() >= deadline or process.poll() is not None:
                raise RuntimeError(
                    "The owned client did not display character paint within the deadline"
                )
            time.sleep(0.01)
        first_observed = (time.perf_counter() - started) * 1000
        first_visible = None
        first_interactive = None
        image.save(output / "native-client.png")
        mica = {
            "pid": process.pid,
            "dwm": attributes,
            "device_size": [client.width, client.height],
            "logical_size": [client.width / client.scale, client.height / client.scale],
            "dpi": client.dpi,
            "first_owned_capture_with_character_ms": first_observed,
            "observation_method": "PrintWindow capture; this is not compositor first-paint timing",
            "launch_command": command,
            "placement": native.placement(hwnd, process.pid, cursor_work),
        }
        write_json(output / "mica.json", mica)
        if arguments.compositor:
            result: dict[str, object] = {"captured": False}
            try:
                visible, observed = observe_compositor(native, hwnd, process, deadline)
            except Exception as error:
                result.update(error_type=type(error).__name__, reason=str(error))
                write_json(output / "compositor.json", result)
                raise
            first_visible = (observed - started) * 1000
            visible.save(output / "native-compositor.png")
            result.update(occluded=False, captured=True,
                          first_owned_compositor_capture_with_character_ms=first_visible)
            write_json(output / "compositor.json", result)
        if arguments.input:
            before = rgb_pixel(
                image, (round(30 * client.scale), round(400 * client.scale))
            )
            native.post(hwnd, process.pid, 0x0100, 9, 0x000F0001)
            native.post(hwnd, process.pid, 0x0101, 9, 0xC00F0001)
            toggle_deadline = min(deadline, time.perf_counter() + 1.0)
            changed = False
            while time.perf_counter() < toggle_deadline and process.poll() is None:
                collapsed = native.capture(hwnd, process.pid)
                after = rgb_pixel(
                    collapsed, (round(30 * client.scale), round(400 * client.scale))
                )
                if max(abs(left - right) for left, right in zip(before, after)) > 5:
                    first_interactive = (time.perf_counter() - started) * 1000
                    collapsed.save(output / "native-collapsed.png")
                    changed = True
                    break
                time.sleep(0.01)
            write_json(
                output / "input.json", {"keyboard_toggle_changed_console": changed}
            )
            if not changed:
                raise RuntimeError(
                    "Native Tab input did not collapse the console within the deadline"
                )
            if arguments.startup:
                startup = startup_decision(first_visible, first_interactive)
                write_json(output / "startup.json", startup)
                if not startup["accepted"]:
                    raise RuntimeError("External visible/interactive startup exceeds S0 targets")
            if arguments.accessibility:
                records = accessibility_snapshot(native, hwnd, process.pid, action="Focus", name=arguments.console_name)
                write_json(output / "accessibility.json", records)
                accessible_controls(records, process.pid, (arguments.console_name, arguments.close_name))
                button = next(record for record in records if record["name"] == arguments.console_name)
                if button["has_keyboard_focus"] is not True:
                    raise RuntimeError("Native accessible disclosure did not receive keyboard focus")
                native.post(button["window_handle"], process.pid, 0x0100, 32, 0x00390001)
                native.post(button["window_handle"], process.pid, 0x0101, 32, 0xC0390001)
                expanded = observe_disclosure(native, hwnd, process, after, client.scale, time.perf_counter() + 1)
                expanded.save(output / "native-accessible-space-expanded.png")
                expanded_pixel = rgb_pixel(expanded, (round(30 * client.scale), round(400 * client.scale)))
                expanded_controls = accessibility_snapshot(native, hwnd, process.pid, action="Invoke", name=arguments.expanded_console_name)
                accessible_controls(expanded_controls, process.pid, (arguments.expanded_console_name, arguments.close_name))
                observe_disclosure(native, hwnd, process, expanded_pixel, client.scale, time.perf_counter() + 1)
                write_json(output / "accessible-actions.json", {"focus_observed": True, "space_expanded_console": True,
                                                               "invoke_collapsed_console": True})
            caption_bounds = (
                round(512 * client.scale),
                0,
                round(558 * client.scale),
                round(32 * client.scale),
            )
            with NativePointer(native, hwnd, process.pid) as pointer:
                pointer.move((489, 60))
                before_cancel = native.capture(hwnd, process.pid).crop(caption_bounds).tobytes()
                pointer.move((535, 16))
                pointer.press()
                observe_caption(native, hwnd, process, before_cancel, caption_bounds, restored=False)
                pointer.move((489, 60))
                pointer.release()
                observe_caption(native, hwnd, process, before_cancel, caption_bounds, restored=True)
                cancelled_close_kept_window_open = True
                if arguments.accessibility:
                    accessibility_snapshot(native, hwnd, process.pid, action="Invoke", name=arguments.close_name)
                else:
                    pointer.move((535, 16))
                    pointer.press()
                    pointer.release()
        stdout, stderr = process.communicate(timeout=arguments.duration_ms / 1000 + 5)
        elapsed = (time.perf_counter() - started) * 1000
        write_json(
            output / "process.json",
            {
                "pid": process.pid,
                "exit_code": process.returncode,
                "wall_elapsed_ms": elapsed,
                "stdout": stdout.decode(errors="replace"),
                "stderr": stderr.decode(errors="replace"),
            },
        )
        if process.returncode != 0:
            raise RuntimeError(f"Native proof exited with status {process.returncode}")
        submission_data = json.loads(submission.read_text(encoding="utf-8"))
        if arguments.input:
            closed_before_deadline = submission_data["elapsed_ms"] < arguments.duration_ms
            write_json(
                output / "input.json",
                {
                    "keyboard_toggle_changed_console": True,
                    "cancelled_close_kept_window_open": cancelled_close_kept_window_open,
                    "close_pressed_feedback_observed": True,
                    "close_cancel_neutral_feedback_observed": True,
                    "native_close_exited_before_demo_deadline": closed_before_deadline,
                    "accessible_close_invoked": arguments.accessibility,
                },
            )
            if not closed_before_deadline:
                raise RuntimeError("Native close action did not exit before the demonstration deadline")
        print(
            json.dumps(
                {
                    "output": str(output),
                    "material": attributes["backdrop"],
                    "submission": submission_data,
                },
                indent=2,
            )
        )
    except Exception as error:
        write_json(output / "failure.json", {"pid": process.pid, "error": str(error),
                                             "error_type": type(error).__name__})
        raise
    finally:
        if process.poll() is None:
            process.terminate()
            process.communicate(timeout=5)


def main() -> int:
    """Parse portable paths and reject unsupported platforms before loading native libraries."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--duration-ms", type=int, default=3000)
    parser.add_argument("--locale", default="en", choices=("en", "zh-Hans", "ja", "ko", "es"))
    parser.add_argument("--light", action="store_true")
    parser.add_argument("--reduced-motion", action="store_true")
    parser.add_argument("--input", action="store_true")
    parser.add_argument("--compositor", action="store_true")
    parser.add_argument("--startup", action="store_true", help="Enforce external 500 ms visible/750 ms interactive targets")
    parser.add_argument("--accessibility", action="store_true", help="Inspect actual owned UI Automation caption buttons")
    parser.add_argument("--console-name", default="Show Comfy output", help="Expected collapsed accessible caption name in the active locale")
    parser.add_argument("--expanded-console-name", default="Hide Comfy output", help="Expected expanded accessible caption name in the active locale")
    parser.add_argument("--close-name", default="Cancel loading", help="Expected accessible close name in the active locale")
    arguments = parser.parse_args()
    if sys.platform != "win32":
        parser.error("This native qualification adapter requires Windows")
    if arguments.duration_ms < 1000:
        parser.error(
            "--duration-ms must allow at least 1000 ms for bounded native qualification"
        )
    if arguments.startup and not (arguments.compositor and arguments.input):
        parser.error("--startup requires --compositor and --input external observations")
    if arguments.accessibility and not arguments.input:
        parser.error("--accessibility requires --input to observe collapsed controls")
    if arguments.accessibility and arguments.duration_ms < 15000:
        parser.error("--accessibility requires --duration-ms >= 15000 for bounded external actions")
    qualify(arguments)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
