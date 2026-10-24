# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Retain exact native process lifetimes for off-screen qualification and short invocations."""
import ctypes
from ctypes import wintypes
from tools.splash_parity.startup_windows import ReferenceProcesses


def declare(library, name, arguments, result):
    function = getattr(library, name)
    function.argtypes, function.restype = arguments, result
    return function


def require(result):
    if not result:
        raise ctypes.WinError(ctypes.get_last_error())
    return result


class ProcessFamily:
    """One fixture owns process handles through observation and checked retirement."""
    def __init__(self):
        self.processes = ReferenceProcesses()
        self.kernel = self.processes.kernel
        self.handles = {}
        self.creations = {}
        self.parents = {}

    def retain_root(self, original):
        """Retain a second handle only after matching a caller-owned original process object."""
        pid, creation = self.processes.lifetime_identity(original)
        if pid in self.handles:
            raise RuntimeError("Fixture root is already retained")
        handle = self.processes.open(pid)
        try:
            if self.processes.lifetime_identity(handle) != (pid, creation):
                raise RuntimeError("Fixture root lifetime changed before retention")
        except BaseException:
            self.processes.close(handle)
            raise
        self.handles[pid], self.creations[pid] = handle, creation
        return pid

    def import_family(self, report, creator, entry):
        """Duplicate only native process capabilities held by this fixture's original creator."""
        if (creator not in self.handles or report.get('creator_pid') != creator
                or report.get('entry_pid') != entry or len(report.get('handles', {})) > 256):
            raise RuntimeError("Process receipt does not belong to the retained creator")
        for pid in report.get('alive', []):
            creation = report['process_creation_filetimes'][str(pid)]
            if pid == creator or creation < self.creations[creator]:
                raise RuntimeError("Process receipt predates its original creator")
            if pid in self.handles:
                if self.creations[pid] != creation:
                    raise RuntimeError("Process receipt conflicts with retained custody")
                continue
            raw = report['handles'][str(pid)]
            if type(raw) is not int or not 0 < raw < ctypes.c_void_p(-1).value:
                raise RuntimeError("Creator process capability is invalid")
            try:
                handle = self.duplicate_handle(self.handles[creator], raw)
            except OSError as cause:
                if cause.winerror in (6, 87):
                    # The creator can retire after publishing its final frame. A later frame,
                    # never this missing capability, must prove that process's actual exit.
                    continue
                raise
            try:
                if self.processes.lifetime_identity(handle) != (pid, creation):
                    raise RuntimeError("Transferred process lifetime differs from its receipt")
            except BaseException:
                self.processes.close(handle)
                raise
            self.handles[pid], self.creations[pid] = handle, creation
            self.parents[pid] = creator if pid == entry else report['parents'][str(pid)]

    def duplicate_handle(self, creator, source):
        """Acquire an independent process reference from the retained creator, without PID lookup."""
        duplicate = declare(self.kernel, "DuplicateHandle", [wintypes.HANDLE, wintypes.HANDLE,
            wintypes.HANDLE, ctypes.POINTER(wintypes.HANDLE), wintypes.DWORD, wintypes.BOOL,
            wintypes.DWORD], wintypes.BOOL)
        result = wintypes.HANDLE()
        require(duplicate(creator, source, wintypes.HANDLE(-1), ctypes.byref(result), 0, False, 2))
        return result.value

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
                try:
                    actual, creation = self.processes.lifetime_identity(handle)
                except BaseException:
                    self.processes.close(handle)
                    raise
                if actual != pid:
                    self.processes.close(handle)
                    raise RuntimeError("Descendant process identity changed during admission")
                if creation < self.creations[parent]:
                    # An older unrelated orphan can retain a reused numeric parent PID.
                    self.processes.close(handle)
                    continue
                self.handles[pid], self.creations[pid] = handle, creation
                self.parents[pid] = parent
                queue.append(pid)

    def descendants(self, root):
        """Return only descendants admitted under retained, creation-checked process custody."""
        result = set()
        queue = [root]
        for parent in queue:
            children = [pid for pid, ancestor in self.parents.items() if ancestor == parent]
            result.update(children)
            queue.extend(children)
        return result

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
        if failures:
            raise RuntimeError(f"Offscreen fixture cleanup encountered {len(failures)} failures") from failures[0]
