# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Qualify an owned Windows splash process, native material, input and CPU submissions."""

from __future__ import annotations

import argparse
import ctypes
import json
import subprocess
import sys
import time
from ctypes import wintypes
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from PIL import Image


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
    process = subprocess.Popen(
        [
            str(arguments.executable),
            "--duration-ms",
            str(arguments.duration_ms),
            "--receipt",
            str(submission),
        ],
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
        image.save(output / "native-client.png")
        mica = {
            "pid": process.pid,
            "dwm": attributes,
            "device_size": [client.width, client.height],
            "logical_size": [client.width / client.scale, client.height / client.scale],
            "dpi": client.dpi,
            "first_owned_capture_with_character_ms": first_observed,
            "observation_method": "PrintWindow capture; this is not compositor first-paint timing",
            "placement": native.placement(hwnd, process.pid, cursor_work),
        }
        write_json(output / "mica.json", mica)
        if arguments.compositor:
            occluded = native.occluded(hwnd, process.pid)
            result: dict[str, object] = {"occluded": occluded, "captured": False}
            if not occluded:
                visible = native.capture(hwnd, process.pid, compositor=True)
                if has_mascot(visible, client.scale):
                    visible.save(output / "native-compositor.png")
                    result["captured"] = True
                else:
                    result["reason"] = (
                        "Compositor did not expose character paint; capture rejected"
                    )
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
            point = (round(16 * client.scale) << 16) | round(535 * client.scale)
            bar_bounds = (
                round(24 * client.scale),
                round(530 * client.scale),
                round(534 * client.scale),
                round(534 * client.scale),
            )
            before_cancel = native.capture(hwnd, process.pid).crop(bar_bounds).tobytes()
            native.post(hwnd, process.pid, 0x0200, 0, point)
            native.post(hwnd, process.pid, 0x0201, 1, point)
            native.post(hwnd, process.pid, 0x02A3, 0, 0)
            native.post(hwnd, process.pid, 0x0202, 0, point)
            cancel_deadline = min(deadline, time.perf_counter() + 0.5)
            cancelled_close_kept_window_open = False
            while time.perf_counter() < cancel_deadline and process.poll() is None:
                changed_bar = (
                    native.capture(hwnd, process.pid).crop(bar_bounds).tobytes()
                )
                if changed_bar != before_cancel:
                    cancelled_close_kept_window_open = True
                    break
                time.sleep(0.005)
            if not cancelled_close_kept_window_open:
                raise RuntimeError(
                    "Interrupted native close gesture did not preserve the running surface"
                )
            native.post(hwnd, process.pid, 0x0200, 0, point)
            native.post(hwnd, process.pid, 0x0201, 1, point)
            native.post(hwnd, process.pid, 0x0202, 0, point)
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
            write_json(
                output / "input.json",
                {
                    "keyboard_toggle_changed_console": True,
                    "cancelled_close_kept_window_open": cancelled_close_kept_window_open,
                    "native_close_exited_before_demo_deadline": submission_data[
                        "elapsed_ms"
                    ]
                    < arguments.duration_ms,
                },
            )
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
    parser.add_argument("--input", action="store_true")
    parser.add_argument("--compositor", action="store_true")
    arguments = parser.parse_args()
    if sys.platform != "win32":
        parser.error("This native qualification adapter requires Windows")
    if arguments.duration_ms < 1000:
        parser.error(
            "--duration-ms must allow at least 1000 ms for bounded native qualification"
        )
    qualify(arguments)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
