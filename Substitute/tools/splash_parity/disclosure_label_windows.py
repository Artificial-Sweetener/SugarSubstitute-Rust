# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Observe an owned native disclosure name after prepared pixels were submitted.

This name is a readiness barrier, not proof of compositor visibility or of the
remaining native submission transaction. Qualification still requires pixels and
a successful terminal outcome. The boundary is injectable for portable tests.
"""
from __future__ import annotations

import ctypes
from ctypes import wintypes
from dataclasses import dataclass


LABELS = ("Show Comfy output", "Hide Comfy output")
CAPACITY = 128


class DisclosureError(RuntimeError):
    """Preserve the failed ownership, text, native-call or deadline operation."""
    def __init__(self, operation, detail):
        self.operation = operation
        super().__init__(f"{operation}: {detail}")


def integer(value, minimum, maximum, name):
    if type(value) is not int or not minimum <= value <= maximum:
        raise DisclosureError("validate_disclosure", f"Invalid {name}")
    return value


@dataclass(frozen=True)
class WindowIdentity:
    hwnd: int
    pid: int
    thread: int
    parent: int
    root: int
    control_id: int
    class_name: str
    visible: bool
    enabled: bool
    minimized: bool


class DisclosureLabel:
    """Retain the verified process and the direct BUTTON1 HWND for one campaign.

    All samples include before/after ownership checks. A fixed WCHAR buffer and
    nonzero message timeout bound each cross-process read. No native activation,
    input attachment, callback hook, or screen capture occurs here.
    """
    def __init__(self, hwnd, pid, creation_ticks, ui_thread_id, clock, *, boundary=None):
        self.hwnd = integer(hwnd, 1, (1 << (ctypes.sizeof(ctypes.c_void_p) * 8)) - 1, "root HWND")
        self.pid = integer(pid, 1, 0xffffffff, "PID")
        self.creation = integer(creation_ticks, 1, 0xffffffffffffffff, "creation time")
        self.thread = integer(ui_thread_id, 1, 0xffffffff, "UI thread")
        self.clock = clock
        self.frequency = integer(clock.frequency, 1, 0x7fffffffffffffff, "QPC frequency")
        self.native = boundary if boundary is not None else _WindowsBoundary()
        self.process = self.child = None
        self.started = self.ready = False
        self.last_read = None

    def __enter__(self):
        if self.started:
            raise DisclosureError("open_disclosure", "Lifetime cannot be entered twice")
        self.started = True
        try:
            self.process = self.native.open_process(self.pid)
            if not self.process:
                raise DisclosureError("open_disclosure", "Process handle was not retained")
            self.child = integer(self.native.child(self.hwnd, 1), 1,
                                 (1 << (ctypes.sizeof(ctypes.c_void_p) * 8)) - 1, "BUTTON HWND")
            self.verify()
            self.ready = True
            return self
        except BaseException as primary:
            try:
                self.close()
            except BaseException as cleanup:
                raise BaseExceptionGroup("Disclosure enrollment and cleanup failed", [primary, cleanup])
            raise

    def verify(self):
        if self.process is None or self.child is None:
            raise DisclosureError("verify_disclosure", "Lifetime is not enrolled")
        identity = self.native.process_identity(self.process)
        if (not isinstance(identity, tuple) or len(identity) != 3
                or type(identity[0]) is not int or type(identity[1]) is not int
                or type(identity[2]) is not bool or identity != (self.pid, self.creation, True)):
            raise DisclosureError("verify_disclosure", "Process identity or liveness changed")
        root, child = self.native.window(self.hwnd), self.native.window(self.child)
        for value in (root, child):
            if (not isinstance(value, WindowIdentity)
                    or any(type(getattr(value, key)) is not int for key in ("hwnd", "pid", "thread", "parent", "root", "control_id"))
                    or any(type(getattr(value, key)) is not bool for key in ("visible", "enabled", "minimized"))
                    or type(value.class_name) is not str):
                raise DisclosureError("verify_disclosure", "Native identity metadata is malformed")
        foreground, registered = self.native.foreground_root(), self.native.child(self.hwnd, 1)
        if (root.hwnd != self.hwnd or root.pid != self.pid or root.thread != self.thread
                or root.root != self.hwnd or not root.visible or not root.enabled or root.minimized
                or type(foreground) is not int or foreground != self.hwnd):
            raise DisclosureError("verify_disclosure", "Root is not the live owned foreground UI surface")
        if (child.hwnd != self.child or child.pid != self.pid or child.thread != self.thread
                or child.parent != self.hwnd or child.root != self.hwnd or child.control_id != 1
                or child.class_name.casefold() != "button" or not child.visible or not child.enabled or child.minimized
                or type(registered) is not int or registered != self.child):
            raise DisclosureError("verify_disclosure", "BUTTON identity, parent, class or state changed")

    def ticks(self):
        return integer(self.clock.read(), 0, 0x7fffffffffffffff, "QPC ticks")

    def read(self, deadline_ticks):
        """Return one bounded name observation with all timing costs retained."""
        self.last_read = dict(hwnd=self.hwnd, child_hwnd=self.child, pid=self.pid,
                              creation_ticks=self.creation, ui_thread_id=self.thread,
                              qpc_frequency=self.frequency, accepted=False)
        try:
            return self._read(deadline_ticks)
        except BaseException as error:
            self.last_read["failure"] = dict(type=type(error).__name__, message=str(error))
            raise

    def _read(self, deadline_ticks):
        if not self.ready:
            raise DisclosureError("read_disclosure", "Reader is outside its owned lifetime")
        integer(deadline_ticks, 1, 0x7fffffffffffffff, "deadline ticks")
        begin = self.ticks()
        self.last_read["begin_ticks"] = begin
        self.verify()
        before_end = self.ticks()
        self.last_read["ownership_before_end_ticks"] = before_end
        native_begin = self.ticks()
        self.last_read["native_call_begin_ticks"] = native_begin
        remaining_ms = (deadline_ticks - native_begin) * 1000 // self.frequency
        if remaining_ms < 1:
            raise DisclosureError("disclosure_deadline", "No nonzero native timeout remains")
        timeout = min(10, remaining_ms)
        self.last_read["native_timeout_ms"] = timeout
        count, text, terminated = self.native.text(self.child, CAPACITY, timeout)
        native_end = self.ticks()
        self.last_read.update(native_call_end_ticks=native_end, returned_wchar_count=count)
        self.verify()
        end = self.ticks()
        self.last_read["end_ticks"] = end
        if not begin <= before_end <= native_begin <= native_end <= end:
            raise DisclosureError("read_disclosure", "QPC brackets reversed")
        if end >= deadline_ticks:
            raise DisclosureError("disclosure_deadline", "Name observation exceeded original deadline")
        integer(count, 1, CAPACITY - 2, "WCHAR count")
        if (type(text) is not str or type(terminated) is not bool or not terminated
                or len(text.encode("utf-16-le")) // 2 != count or text not in LABELS):
            raise DisclosureError("read_disclosure", "Name is unexpected, malformed or truncated")
        self.last_read.update(label=text, accepted=True)
        return dict(self.last_read)

    def close(self):
        """Release only the retained query handle; failed cleanup remains retryable."""
        self.ready = False
        if self.process is not None:
            self.native.close_process(self.process)
            self.process = None

    def __exit__(self, kind, primary, traceback):
        try:
            self.close()
        except BaseException as cleanup:
            if primary is not None:
                raise BaseExceptionGroup("Disclosure observation and cleanup failed", [primary, cleanup])
            raise
        return False


class _WindowsBoundary:
    """Declare exact Win32 ABI at the qualification-only native boundary."""
    def __init__(self):
        self.user = ctypes.WinDLL("user32", use_last_error=True)
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        for call, arguments, result in (
            (self.user.GetDlgItem, [wintypes.HWND, ctypes.c_int], wintypes.HWND),
            (self.user.GetDlgCtrlID, [wintypes.HWND], ctypes.c_int),
            (self.user.GetParent, [wintypes.HWND], wintypes.HWND),
            (self.user.GetAncestor, [wintypes.HWND, wintypes.UINT], wintypes.HWND),
            (self.user.GetForegroundWindow, [], wintypes.HWND),
            (self.user.GetWindowThreadProcessId, [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)], wintypes.DWORD),
            (self.user.GetClassNameW, [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int], ctypes.c_int),
            (self.user.SendMessageTimeoutW, [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM,
                                          wintypes.UINT, wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)], wintypes.LPARAM),
            (self.kernel.OpenProcess, [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
            (self.kernel.CloseHandle, [wintypes.HANDLE], wintypes.BOOL),
            (self.kernel.GetProcessId, [wintypes.HANDLE], wintypes.DWORD),
            (self.kernel.GetProcessTimes, [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4, wintypes.BOOL),
            (self.kernel.WaitForSingleObject, [wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
        ):
            call.argtypes, call.restype = arguments, result
        for name in ("IsWindow", "IsWindowVisible", "IsWindowEnabled", "IsIconic"):
            call = getattr(self.user, name)
            call.argtypes, call.restype = [wintypes.HWND], wintypes.BOOL

    @staticmethod
    def fail(operation):
        raise DisclosureError(operation, f"Native call failed (Windows error {ctypes.get_last_error()})")

    def open_process(self, pid):
        handle = self.kernel.OpenProcess(0x1000 | 0x100000, False, pid)
        if not handle:
            self.fail("open_disclosure_process")
        return handle

    def close_process(self, handle):
        if not self.kernel.CloseHandle(handle):
            self.fail("close_disclosure_process")

    def process_identity(self, handle):
        pid = self.kernel.GetProcessId(handle)
        timestamps = [wintypes.FILETIME() for _ in range(4)]
        if not pid or not self.kernel.GetProcessTimes(handle, *(ctypes.byref(value) for value in timestamps)):
            self.fail("identify_disclosure_process")
        status = self.kernel.WaitForSingleObject(handle, 0)
        if status not in (0, 0x102):
            self.fail("observe_disclosure_process")
        creation = timestamps[0].dwHighDateTime << 32 | timestamps[0].dwLowDateTime
        return pid, creation, status == 0x102

    def child(self, root, control_id):
        child = self.user.GetDlgItem(root, control_id)
        if not child:
            self.fail("find_disclosure_button")
        return child

    def window(self, hwnd):
        if not self.user.IsWindow(hwnd):
            self.fail("identify_disclosure_window")
        pid = wintypes.DWORD()
        thread = self.user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        name = ctypes.create_unicode_buffer(32)
        count = self.user.GetClassNameW(hwnd, name, len(name))
        root = self.user.GetAncestor(hwnd, 2)
        if not thread or not pid.value or not root or not 0 < count < len(name) - 1:
            self.fail("identify_disclosure_window")
        return WindowIdentity(hwnd, pid.value, thread, self.user.GetParent(hwnd) or 0, root,
                              self.user.GetDlgCtrlID(hwnd), name.value,
                              bool(self.user.IsWindowVisible(hwnd)), bool(self.user.IsWindowEnabled(hwnd)),
                              bool(self.user.IsIconic(hwnd)))

    def foreground_root(self):
        foreground = self.user.GetForegroundWindow()
        root = self.user.GetAncestor(foreground, 2) if foreground else None
        if not root:
            self.fail("identify_disclosure_foreground")
        return root

    def text(self, hwnd, capacity, timeout):
        buffer = ctypes.create_unicode_buffer(capacity)
        result = ctypes.c_size_t()
        ctypes.set_last_error(0)
        if not self.user.SendMessageTimeoutW(hwnd, 0x000D, capacity, ctypes.addressof(buffer),
                                            0x0001 | 0x0002 | 0x0020, timeout, ctypes.byref(result)):
            self.fail("read_disclosure_text")
        count = result.value
        if not 0 <= count < capacity:
            raise DisclosureError("read_disclosure_text", "WM_GETTEXT returned an oversized count")
        return count, buffer.value, buffer[count] == "\0"
