# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Own a foreground launch gesture before the externally measured product startup."""

from __future__ import annotations

import ctypes
import os
import time
from ctypes import wintypes

try:
    from .native_windows import NativePointer, NativeWindows
except ImportError:
    from native_windows import NativePointer, NativeWindows


class Message(ctypes.Structure):
    """Match MSG, including its reserved DWORD and native alignment."""

    _fields_ = [("hwnd", wintypes.HWND), ("message", wintypes.UINT),
                ("wparam", ctypes.c_size_t), ("lparam", ctypes.c_ssize_t),
                ("time", wintypes.DWORD), ("point", wintypes.POINT), ("private", wintypes.DWORD)]


def launcher_bounds(work: list[int], scale: float) -> tuple[int, int, int, int]:
    """Choose a small owned launch surface outside the future centered 558-logical-pixel client."""
    width, height = 160, 48
    product = round(558 * scale)
    left, top, right, bottom = work
    product_left, product_top = left + (right - left - product) // 2, top + (bottom - top - product) // 2
    for x, y in ((left + 8, top + 8), (right - width - 8, top + 8),
                 (left + 8, bottom - height - 8), (right - width - 8, bottom - height - 8)):
        if (left <= x and top <= y and x + width <= right and y + height <= bottom
                and (x + width <= product_left or x >= product_left + product
                     or y + height <= product_top or y >= product_top + product)):
            return x, y, width, height
    raise RuntimeError("No owned launcher position fits outside the future centered product client")


class ActivationPointer(NativePointer):
    """Admit only an actual click-to-activate on the launcher's verified unoccluded client."""

    def owned_hit(self, point: tuple[int, int]) -> None:
        """Allow inactive ownership before the gesture while rejecting foreign targets and occlusion."""
        self.native.client(self.hwnd, self.pid)
        hit = self.native.user.WindowFromPoint(wintypes.POINT(*point))
        root = self.native.user.GetAncestor(hit, 2) if hit else 0
        rect = wintypes.RECT()
        self.last_probe = {"owned_hwnd": self.hwnd, "owned_pid": self.pid, "cursor_screen": point,
                           "hit_hwnd": hit or 0, "hit_root_hwnd": root or 0}
        if not self.native.user.GetWindowRect(self.hwnd, ctypes.byref(rect)):
            raise ctypes.WinError(ctypes.get_last_error())
        self.last_probe.update(owned_screen_bounds=[rect.left, rect.top, rect.right, rect.bottom],
                               exposure=self.native.exposure_state(self.hwnd, self.pid))
        if not hit or root != self.hwnd or self.native.occluded(self.hwnd, self.pid):
            raise RuntimeError("The launch activation target is not the unoccluded owned client")


class ForegroundLauncher:
    """Keep one standard native window and its activation on the calling GUI thread."""

    def __init__(self, native: NativeWindows) -> None:
        """Borrow the observer's Win32 boundary; no product handles or foreign windows are modified."""
        self.native, self.pid, self.hwnd = native, os.getpid(), None
        self.receipt: dict[str, object] = {}
        self.hidden = False
        for function, arguments, result in (
            (native.user.CreateWindowExW, [wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD,
                                          ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                          wintypes.HWND, wintypes.HMENU, wintypes.HINSTANCE, ctypes.c_void_p], wintypes.HWND),
            (native.user.DestroyWindow, [wintypes.HWND], wintypes.BOOL),
            (native.user.AllowSetForegroundWindow, [wintypes.DWORD], wintypes.BOOL),
            (native.user.ShowWindow, [wintypes.HWND, ctypes.c_int], wintypes.BOOL),
            (native.user.SetWindowPos, [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int,
                                       ctypes.c_int, ctypes.c_int, wintypes.UINT], wintypes.BOOL),
            (native.user.PeekMessageW, [ctypes.POINTER(Message), wintypes.HWND, wintypes.UINT, wintypes.UINT, wintypes.UINT], wintypes.BOOL),
            (native.user.TranslateMessage, [ctypes.POINTER(Message)], wintypes.BOOL),
            (native.user.DispatchMessageW, [ctypes.POINTER(Message)], ctypes.c_ssize_t),
        ):
            function.argtypes, function.restype = arguments, result

    def pump(self) -> None:
        """Dispatch only this owned standard window's queued messages with a bounded inventory."""
        self.native.client(self.hwnd, self.pid)
        message = Message()
        for _ in range(256):
            if not self.native.user.PeekMessageW(ctypes.byref(message), self.hwnd, 0, 0, 1):
                return
            self.native.user.TranslateMessage(ctypes.byref(message))
            self.native.user.DispatchMessageW(ctypes.byref(message))
            self.native.client(self.hwnd, self.pid)
        raise RuntimeError("Owned launch fixture exceeded its bounded message inventory")

    def activate(self) -> None:
        """Require observed foreground after real mouse input before allowing the product launch."""
        deadline = time.perf_counter() + 2
        before = self.native.exposure_state(self.hwnd, self.pid)
        self.receipt = {"method": "actual SendInput click on owned standard native launch fixture",
                        "before": before, "fixture_position": "outside future centered product client",
                        "fixture_gui_thread": "calling observer thread; standard BUTTON window procedure"}
        with ActivationPointer(self.native, self.hwnd, self.pid) as pointer:
            try:
                client = self.native.client(self.hwnd, self.pid)
                pointer.move((client.width / client.scale / 2, client.height / client.scale / 2))
                pointer.press()
                while True:
                    self.pump()
                    state = self.native.exposure_state(self.hwnd, self.pid)
                    if state["foreground_owned"] and not state["occluding_windows"]:
                        break
                    if time.perf_counter() >= deadline:
                        self.receipt["activation_deadline_state"] = state
                        raise RuntimeError("Actual launch fixture click did not establish owned foreground")
                    time.sleep(.005)
                pointer.release()
                self.pump()
            finally:
                probe = getattr(pointer, "last_probe", None)
                if isinstance(probe, dict):
                    self.receipt["last_physical_hit_probe"] = probe
        after = self.native.exposure_state(self.hwnd, self.pid)
        if not after["foreground_owned"] or after["occluding_windows"]:
            raise RuntimeError("Owned launcher foreground changed before product launch admission")
        self.receipt["before_product_launch"] = after

    def __enter__(self) -> ForegroundLauncher:
        """Create a standard popup without requesting focus, then establish actual gesture ownership."""
        work = self.native.cursor_work_area()
        x, y, width, height = launcher_bounds(work, 1)
        # The system BUTTON procedure owns callbacks and physical hit testing; no Python callback spans native dispatch.
        self.hwnd = self.native.user.CreateWindowExW(0x88, "BUTTON", "Startup qualification", 0x80000000,
                                                      x, y, width, height, None, None, None, None)
        if not self.hwnd:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            self.native.user.ShowWindow(self.hwnd, 8)
            client = self.native.client(self.hwnd, self.pid)
            actual = launcher_bounds(work, client.scale)
            if actual != (x, y, width, height) and not self.native.user.SetWindowPos(self.hwnd, -1, *actual, 0x10):
                raise ctypes.WinError(ctypes.get_last_error())
            self.activate()
            return self
        except BaseException as error:
            self.__exit__(type(error), error, error.__traceback__)
            raise

    def product_exposed(self) -> None:
        """Hide only the owned launcher after the product already owns actual foreground exposure."""
        self.native.client(self.hwnd, self.pid)
        self.native.user.ShowWindow(self.hwnd, 0)
        if self.native.user.IsWindowVisible(self.hwnd):
            raise RuntimeError("Owned launch fixture did not hide after product exposure")
        self.hidden = True

    def allow_product(self, pid: int) -> None:
        """Grant only the verified launched PID permission while the launch fixture remains foreground."""
        if pid <= 0 or pid == self.pid:
            raise ValueError("Foreground permission requires the distinct verified launched product PID")
        state = self.native.exposure_state(self.hwnd, self.pid)
        if state.get("foreground_pid") == pid:
            self.receipt["activation_permission"] = {"api": None, "verified_product_pid": pid,
                                                      "policy": "Verified product already activated itself from the foreground launch"}
            return
        if not state["foreground_owned"] or state["occluding_windows"]:
            raise RuntimeError("Launcher lost foreground before granting the verified product activation permission")
        if not self.native.user.AllowSetForegroundWindow(pid):
            raise ctypes.WinError(ctypes.get_last_error())
        self.receipt["activation_permission"] = {"api": "AllowSetForegroundWindow", "verified_product_pid": pid,
                                                  "policy": "Permission only; product must expose its own window without forced activation"}

    def __exit__(self, kind: object, error: BaseException | None, traceback: object) -> bool:
        """Destroy the owned fixture on its creating thread, preserving any cleanup failure."""
        if self.hwnd is not None:
            if not self.native.owns(self.hwnd, self.pid):
                cleanup = RuntimeError("Launcher fixture lost HWND ownership before teardown")
            elif not self.native.user.DestroyWindow(self.hwnd):
                cleanup = ctypes.WinError(ctypes.get_last_error())
            else:
                self.hwnd = None
                return False
            if error is not None:
                raise BaseExceptionGroup("Launch observation and fixture teardown failed", [error, cleanup])
            raise cleanup
        return False
