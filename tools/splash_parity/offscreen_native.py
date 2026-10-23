# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Own native QA processes and windows exclusively on a fresh inactive desktop."""
from __future__ import annotations

import ctypes
import subprocess
import uuid
from ctypes import wintypes
from pathlib import Path

import win32con
import win32gui
import win32process

from tools.splash_parity.startup_windows import ReferenceProcesses


class StartupInfo(ctypes.Structure):
    """Match STARTUPINFOW, including pointer-width standard handles."""
    _fields_ = [
        ("cb", wintypes.DWORD), ("reserved", wintypes.LPWSTR),
        ("desktop", wintypes.LPWSTR), ("title", wintypes.LPWSTR),
        ("x", wintypes.DWORD), ("y", wintypes.DWORD),
        ("width", wintypes.DWORD), ("height", wintypes.DWORD),
        ("columns", wintypes.DWORD), ("rows", wintypes.DWORD),
        ("fill", wintypes.DWORD), ("flags", wintypes.DWORD),
        ("show", wintypes.WORD), ("reserved_size", wintypes.WORD),
        ("reserved_bytes", ctypes.c_void_p), ("stdin", wintypes.HANDLE),
        ("stdout", wintypes.HANDLE), ("stderr", wintypes.HANDLE),
    ]


class ExtendedStartupInfo(ctypes.Structure):
    """Retain the parent-process attribute alongside the explicit private desktop."""
    _fields_ = [("startup", StartupInfo), ("attributes", ctypes.c_void_p)]


class ProcessInfo(ctypes.Structure):
    """Own both handles returned by CreateProcessW."""
    _fields_ = [("process", wintypes.HANDLE), ("thread", wintypes.HANDLE),
                ("pid", wintypes.DWORD), ("tid", wintypes.DWORD)]


def declare(library, name, arguments, result):
    function = getattr(library, name)
    function.argtypes, function.restype = arguments, result
    return function


def require(result):
    if not result:
        raise ctypes.WinError(ctypes.get_last_error())
    return result


class PrivateDesktop:
    """Never request switch rights or operate on the user's input desktop."""
    def __init__(self):
        self.user = ctypes.WinDLL("user32", use_last_error=True)
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.processes = ReferenceProcesses()
        self.handles = {}
        self.creations = {}
        self.callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
        self.create_desktop = declare(self.user, "CreateDesktopW", [wintypes.LPCWSTR,
            wintypes.LPCWSTR, ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
            ctypes.c_void_p], wintypes.HANDLE)
        self.close_desktop = declare(self.user, "CloseDesktop", [wintypes.HANDLE], wintypes.BOOL)
        self.enumerate = declare(self.user, "EnumDesktopWindows", [wintypes.HANDLE,
            self.callback_type, wintypes.LPARAM], wintypes.BOOL)
        self.shell_window = declare(self.user, "GetShellWindow", [], wintypes.HWND)
        self.initialize_attributes = declare(self.kernel, "InitializeProcThreadAttributeList",
            [ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
             ctypes.POINTER(ctypes.c_size_t)], wintypes.BOOL)
        self.update_attribute = declare(self.kernel, "UpdateProcThreadAttribute",
            [ctypes.c_void_p, wintypes.DWORD, ctypes.c_size_t, ctypes.c_void_p,
             ctypes.c_size_t, ctypes.c_void_p, ctypes.c_void_p], wintypes.BOOL)
        self.delete_attributes = declare(self.kernel, "DeleteProcThreadAttributeList",
            [ctypes.c_void_p], None)
        self.create_process = declare(self.kernel, "CreateProcessW", [wintypes.LPCWSTR,
            wintypes.LPWSTR, ctypes.c_void_p, ctypes.c_void_p, wintypes.BOOL,
            wintypes.DWORD, ctypes.c_void_p, wintypes.LPCWSTR,
            ctypes.POINTER(ExtendedStartupInfo), ctypes.POINTER(ProcessInfo)], wintypes.BOOL)
        self.name = "SugarSubstitute-QA-" + uuid.uuid4().hex
        # READOBJECTS | CREATEWINDOW | ENUMERATE | WRITEOBJECTS, without SWITCHDESKTOP.
        self.desktop = require(self.create_desktop(self.name, None, None, 0, 0xC3, None))

    def launch(self, executable: Path, directory: Path, arguments=()) -> int:
        """Use an ordinary shell parent and an explicit private desktop, with no shell gesture."""
        shell = require(self.shell_window())
        _, shell_pid = win32process.GetWindowThreadProcessId(shell)
        parent = require(self.processes.kernel.OpenProcess(0x80, False, shell_pid))
        size = ctypes.c_size_t()
        self.initialize_attributes(None, 1, 0, ctypes.byref(size))
        storage = ctypes.create_string_buffer(size.value)
        initialized = False
        try:
            require(self.initialize_attributes(storage, 1, 0, ctypes.byref(size)))
            initialized = True
            parent_handle = wintypes.HANDLE(parent)
            require(self.update_attribute(storage, 0, 0x20000, ctypes.byref(parent_handle),
                                          ctypes.sizeof(parent_handle), None, None))
            startup = ExtendedStartupInfo()
            startup.startup.cb = ctypes.sizeof(startup)
            startup.startup.desktop = "WinSta0\\" + self.name
            startup.attributes = ctypes.cast(storage, ctypes.c_void_p)
            result = ProcessInfo()
            command = ctypes.create_unicode_buffer(subprocess.list2cmdline(
                [str(executable), *arguments]))
            require(self.create_process(str(executable), command, None, None, False,
                0x80000 | 0x400 | (0x08000000 if executable.name.lower().startswith("python") else 0), None, str(directory), ctypes.byref(startup), ctypes.byref(result)))
            self.processes.close(result.thread)
            self.handles[result.pid] = result.process
            self.creations[result.pid] = self.processes.lifetime_identity(result.process)[1]
            return result.pid
        finally:
            if initialized:
                self.delete_attributes(storage)
            self.processes.close(parent)

    def discover(self):
        """Retain descendant handles before parents exit; handle ownership prevents PID reuse."""
        ancestry = self.processes.ancestry()
        queue = list(self.handles)
        for parent in queue:
            if len(self.handles) > 256:
                raise RuntimeError("Test process family exceeded 256 retained processes")
            for pid, _ in ancestry.get(parent, ()):
                if pid in self.handles:
                    continue
                try:
                    handle = self.processes.open(pid)
                except OSError as cause:
                    if cause.winerror == 87:  # A snapshot child has already exited.
                        continue
                    raise
                actual, creation = self.processes.lifetime_identity(handle)
                if actual != pid:
                    self.processes.close(handle)
                    raise RuntimeError("Descendant process identity changed during admission")
                if creation < self.creations[parent]:
                    # An older unrelated orphan can retain a reused numeric parent PID.
                    self.processes.close(handle)
                    continue
                self.handles[pid], self.creations[pid] = handle, creation
                queue.append(pid)

    def alive(self):
        return [pid for pid, handle in self.handles.items() if self.processes.alive(handle)]

    def observe_images(self, report):
        """Record images or verified terminal metadata loss against retained process lifetimes."""
        images = report.setdefault("process_images", {})
        unavailable = report.setdefault("unavailable_process_images", {})
        for pid, handle in self.handles.items():
            if pid in images or pid in unavailable:
                continue
            try:
                actual, executable, creation = self.processes.identity(handle)
            except OSError as cause:
                # Image metadata can disappear before the retained process's exit event signals.
                if cause.winerror not in (5, 31) or not self.processes.wait_exited(handle, 100):
                    raise
                actual, creation = self.processes.lifetime_identity(handle)
                if actual != pid or creation != self.creations[pid]:
                    raise RuntimeError("Exited process lifetime differs from retained custody") from cause
                unavailable[pid] = {
                    "winerror": cause.winerror,
                    "creation_100ns": creation,
                    "exited": True,
                    "exit_code": self.exit_codes()[pid],
                    "reason": "Image metadata retired after original retained process exited",
                }
            else:
                if actual != pid or creation != self.creations[pid]:
                    raise RuntimeError("Image observation differs from retained process lifetime")
                images[pid] = str(executable)

    def stop(self, pid):
        """Fault only a process whose original native handle belongs to this test."""
        self.processes.stop(self.handles[pid])

    def suspend(self, pid):
        """Freeze a root created by this fixture to exercise its real watchdog."""
        self.processes.suspend(self.handles[pid])

    def windows(self):
        """Read only this desktop's application windows; keep callback errors observable."""
        found, failures = [], []
        @self.callback_type
        def collect(hwnd, _):
            try:
                _, pid = win32process.GetWindowThreadProcessId(hwnd)
                title = win32gui.GetWindowText(hwnd) if pid in self.handles else ""
                if title:
                    client = win32gui.GetClientRect(hwnd)
                    found.append(dict(hwnd=int(hwnd), pid=pid, title=title,
                        cls=win32gui.GetClassName(hwnd),
                        visible=bool(win32gui.IsWindowVisible(hwnd)),
                        rect=win32gui.GetWindowRect(hwnd), client=client,
                        splash=title == "SugarSubstitute" and client[2] == client[3] and 400 <= client[2] <= 1200,
                        console=self.control_visible(hwnd, 4),
                        retry=self.control_visible(hwnd, 3)))
            except win32gui.error as cause:
                if win32gui.IsWindow(hwnd):
                    failures.append(cause)
            return True
        ctypes.set_last_error(0)
        if not self.enumerate(self.desktop, collect, 0) and ctypes.get_last_error():
            raise ctypes.WinError(ctypes.get_last_error())
        if failures:
            raise failures[0]
        return found

    def assert_window(self, hwnd):
        if hwnd not in {value["hwnd"] for value in self.windows()}:
            raise RuntimeError("Window is not owned by this test on its inactive desktop")

    def move(self, hwnd, x, y):
        self.assert_window(hwnd)
        win32gui.SetWindowPos(hwnd, 0, x, y, 0, 0,
            win32con.SWP_NOSIZE | win32con.SWP_NOACTIVATE | win32con.SWP_NOZORDER)

    def close_window(self, hwnd):
        self.assert_window(hwnd)
        win32gui.PostMessage(hwnd, win32con.WM_CLOSE, 0, 0)

    def click_control(self, hwnd, identifier):
        """Post a native button activation only to a verified private test window."""
        self.assert_window(hwnd)
        control = win32gui.GetDlgItem(hwnd, identifier)
        if not control or not win32gui.IsWindowVisible(control):
            raise RuntimeError("Requested native control is not visible")
        win32gui.PostMessage(control, win32con.BM_CLICK, 0, 0)

    def console_tail(self, hwnd):
        """Read bounded diagnostic text through Windows' cross-process WM_GETTEXT marshalling."""
        self.assert_window(hwnd)
        control = win32gui.GetDlgItem(hwnd, 4)
        if not control or not win32gui.IsWindowVisible(control):
            return None
        send = declare(self.user, "SendMessageTimeoutW", [wintypes.HWND, wintypes.UINT,
            wintypes.WPARAM, wintypes.LPARAM, wintypes.UINT, wintypes.UINT,
            ctypes.POINTER(ctypes.c_size_t)], wintypes.LPARAM)
        buffer = ctypes.create_unicode_buffer(262145)
        result = ctypes.c_size_t()
        require(send(control, win32con.WM_GETTEXT, len(buffer), ctypes.addressof(buffer),
                     3, 1000, ctypes.byref(result)))
        return buffer.value[-8192:]

    def exit_codes(self):
        """Read original process objects, including terminal status after window retirement."""
        get = declare(self.kernel, "GetExitCodeProcess", [wintypes.HANDLE,
            ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL)
        result = {}
        for pid, handle in self.handles.items():
            code = wintypes.DWORD()
            require(get(handle, ctypes.byref(code)))
            result[pid] = code.value
        return result

    def caption_hits(self, hwnd):
        """Probe native titlebar hit testing on owned descendants without mouse input."""
        self.assert_window(hwnd)
        children = [hwnd]
        win32gui.EnumChildWindows(hwnd, lambda child, _: children.append(child), None)
        send = declare(self.user, "SendMessageTimeoutW", [wintypes.HWND, wintypes.UINT,
            wintypes.WPARAM, wintypes.LPARAM, wintypes.UINT, wintypes.UINT,
            ctypes.POINTER(ctypes.c_size_t)], wintypes.LPARAM)
        dpi = declare(self.user, "GetDpiForWindow", [wintypes.HWND], wintypes.UINT)(hwnd)
        width = win32gui.GetClientRect(hwnd)[2]
        hits = []
        for fraction in (.5, .65, .75):
            x, y = win32gui.ClientToScreen(hwnd, (int(width * fraction), round(17 * dpi / 96)))
            packed = (x & 0xffff) | ((y & 0xffff) << 16)
            for child in children:
                if not win32gui.IsWindowVisible(child):
                    continue
                left, top, right, bottom = win32gui.GetWindowRect(child)
                if not (left <= x < right and top <= y < bottom):
                    continue
                result = ctypes.c_size_t()
                require(send(child, win32con.WM_NCHITTEST, 0, packed, 3, 1000, ctypes.byref(result)))
                hits.append(dict(fraction=fraction, native_class=win32gui.GetClassName(child),
                                 hit=ctypes.c_ssize_t(result.value).value))
        return hits

    @staticmethod
    def control_visible(hwnd, identifier):
        try:
            control = win32gui.GetDlgItem(hwnd, identifier)
            return bool(control and win32gui.IsWindowVisible(control))
        except win32gui.error:
            return False

    @staticmethod
    def dialog_text(hwnd):
        texts = []
        win32gui.EnumChildWindows(hwnd,
            lambda child, _: texts.append(win32gui.GetWindowText(child)), None)
        return texts

    def close(self):
        """Failure cleanup stays on retained process handles; success requires prior natural exit."""
        failures = []
        try:
            try:
                self.discover()
            except Exception as cause:
                failures.append(cause)
            for handle in reversed(list(self.handles.values())):
                try:
                    self.processes.stop(handle)
                except Exception as cause:
                    failures.append(cause)
        finally:
            for handle in self.handles.values():
                try:
                    self.processes.close(handle)
                except Exception as cause:
                    failures.append(cause)
            self.handles.clear()
            require(self.close_desktop(self.desktop))
        if failures:
            raise RuntimeError(f"Offscreen fixture cleanup encountered {len(failures)} failures") from failures[0]
