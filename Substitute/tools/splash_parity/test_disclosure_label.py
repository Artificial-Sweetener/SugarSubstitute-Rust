# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Exercise owned disclosure reads without claiming native Windows qualification."""
from __future__ import annotations

import ctypes
import unittest
from dataclasses import replace
from types import SimpleNamespace
from unittest.mock import patch

try:
    from . import disclosure_label_windows as owner
except ImportError:
    import disclosure_label_windows as owner


class Clock:
    frequency = 1000000
    def __init__(self):
        self.current = 100
    def read(self):
        self.current += 100
        return self.current


class Boundary:
    def __init__(self, clock):
        self.clock, self.events = clock, []
        self.process = (55, 99, True)
        self.root = owner.WindowIdentity(11, 55, 71, 0, 11, 0, "host", True, True, False)
        self.button = owner.WindowIdentity(12, 55, 71, 11, 11, 1, "Button", True, True, False)
        self.button_hwnd, self.foreground = 12, 11
        self.returned = (len(owner.LABELS[0]), owner.LABELS[0], True)
        self.after_text = lambda: None
        self.open_fail, self.close_fail, self.text_fail = False, False, False
    def open_process(self, pid):
        self.events.append(("open", pid))
        if self.open_fail:
            raise owner.DisclosureError("open", "original native cause")
        return 10
    def close_process(self, handle):
        self.events.append(("close", handle))
        if self.close_fail:
            raise owner.DisclosureError("close", "native cleanup cause")
    def process_identity(self, handle):
        self.events.append(("process", handle))
        return self.process
    def child(self, root, control_id):
        self.events.append(("child", root, control_id))
        return self.button_hwnd
    def window(self, hwnd):
        self.events.append(("window", hwnd))
        return self.root if hwnd == 11 else self.button
    def foreground_root(self):
        return self.foreground
    def text(self, child, capacity, timeout):
        self.events.append(("text", child, capacity, timeout))
        if self.text_fail:
            raise owner.DisclosureError("read_disclosure_text", "timeout or generic native failure")
        self.after_text()
        return self.returned


def fixture():
    clock = Clock()
    boundary = Boundary(clock)
    reader = owner.DisclosureLabel(11, 55, 99, 71, clock, boundary=boundary)
    return clock, boundary, reader


class DisclosureTests(unittest.TestCase):
    def test_read_retains_owned_handle_identity_and_actual_complete_brackets(self):
        clock, boundary, reader = fixture()
        with reader:
            sample = reader.read(clock.current + clock.frequency)
            self.assertEqual(sample["label"], owner.LABELS[0])
            self.assertEqual((sample["hwnd"], sample["child_hwnd"], sample["pid"],
                              sample["creation_ticks"], sample["ui_thread_id"]), (11, 12, 55, 99, 71))
            brackets = [sample[key] for key in ("begin_ticks", "ownership_before_end_ticks",
                                                "native_call_begin_ticks", "native_call_end_ticks", "end_ticks")]
            self.assertEqual(brackets, sorted(brackets))
            call = next(event for event in boundary.events if event[0] == "text")
            self.assertEqual(call, ("text", 12, 128, 10))
            index = boundary.events.index(call)
            self.assertTrue(any(event[0] == "process" for event in boundary.events[:index]))
            self.assertTrue(any(event[0] == "process" for event in boundary.events[index + 1:]))
        self.assertEqual(boundary.events[-1], ("close", 10))
        self.assertIsNone(reader.process)
        self.assertRaises(owner.DisclosureError, reader.read, 1000000)
        self.assertRaises(owner.DisclosureError, reader.__enter__)

    def test_every_root_child_process_and_foreground_identity_is_checked_on_both_sides(self):
        mutations = [
            lambda b: setattr(b, "process", (56, 99, True)),
            lambda b: setattr(b, "process", (55, 100, True)),
            lambda b: setattr(b, "process", (55, 99, False)),
            lambda b: setattr(b, "foreground", 77),
            lambda b: setattr(b, "button_hwnd", 13),
        ]
        for key, value in (("hwnd", 13), ("pid", 56), ("thread", 72), ("parent", 77),
                           ("root", 77), ("control_id", 2), ("class_name", "Edit"),
                           ("visible", False), ("enabled", False), ("minimized", True)):
            mutations.append(lambda b, key=key, value=value: setattr(b, "button", replace(b.button, **{key: value})))
        for key, value in (("hwnd", 77), ("pid", 56), ("thread", 72), ("root", 77),
                           ("visible", False), ("enabled", False), ("minimized", True)):
            mutations.append(lambda b, key=key, value=value: setattr(b, "root", replace(b.root, **{key: value})))
        for after in (False, True):
            for index, mutation in enumerate(mutations):
                with self.subTest(after=after, index=index):
                    clock, boundary, reader = fixture()
                    with reader:
                        if after:
                            boundary.after_text = lambda: mutation(boundary)
                        else:
                            mutation(boundary)
                        self.assertRaises(owner.DisclosureError, reader.read, clock.current + clock.frequency)
                    self.assertEqual(sum(event[0] == "text" for event in boundary.events), int(after))

    def test_unexpected_empty_truncated_unterminated_and_noninteger_text_never_qualify(self):
        cases = [(0, "", True), (127, "x" * 127, True), (128, "x" * 128, True),
                 (True, owner.LABELS[0], True), (16.0, owner.LABELS[0], True),
                 (None, owner.LABELS[0], True), (len(owner.LABELS[0]), owner.LABELS[0], False),
                 (len(owner.LABELS[0]), owner.LABELS[0], 1), (4, "wrong", True),
                 (len(owner.LABELS[0]) + 1, owner.LABELS[0], True)]
        for returned in cases:
            with self.subTest(returned=returned):
                clock, boundary, reader = fixture()
                with reader:
                    boundary.returned = returned
                    self.assertRaises(owner.DisclosureError, reader.read, clock.current + clock.frequency)

    def test_deadline_bounds_native_timeout_and_rejects_late_valid_label(self):
        clock, boundary, reader = fixture()
        with reader:
            sample = reader.read(clock.current + 2300)
            self.assertEqual(sample["native_timeout_ms"], 2)
            boundary.after_text = lambda: setattr(clock, "current", clock.current + clock.frequency)
            with self.assertRaisesRegex(owner.DisclosureError, "disclosure_deadline"):
                reader.read(clock.current + clock.frequency)
        clock, boundary, reader = fixture()
        with reader:
            self.assertRaises(owner.DisclosureError, reader.read, clock.current + 300)
            self.assertFalse(any(event[0] == "text" for event in boundary.events))

    def test_native_timeout_and_cleanup_causes_remain_typed_and_recoverable(self):
        clock, boundary, reader = fixture()
        boundary.text_fail, boundary.close_fail = True, True
        with self.assertRaises(BaseExceptionGroup) as raised:
            with reader:
                reader.read(clock.current + clock.frequency)
        self.assertEqual([error.operation for error in raised.exception.exceptions], ["read_disclosure_text", "close"])
        self.assertEqual(reader.process, 10)
        boundary.close_fail = False
        reader.close()
        self.assertIsNone(reader.process)

    def test_enrollment_failure_cleanup_and_failed_open_are_observed(self):
        clock, boundary, reader = fixture()
        boundary.button_hwnd, boundary.close_fail = 0, True
        self.assertRaises(BaseExceptionGroup, reader.__enter__)
        self.assertEqual(reader.process, 10)
        boundary.close_fail = False
        reader.close()
        clock, boundary, reader = fixture()
        boundary.open_fail = True
        self.assertRaises(owner.DisclosureError, reader.__enter__)
        self.assertFalse(any(event[0] == "close" for event in boundary.events))

    def test_invalid_identity_and_qpc_metadata_reject(self):
        for field, value in (("pid", True), ("creation_ticks", 0), ("ui_thread_id", 1.0), ("hwnd", 0)):
            clock, boundary, _ = fixture()
            values = dict(hwnd=11, pid=55, creation_ticks=99, ui_thread_id=71, clock=clock, boundary=boundary)
            values[field] = value
            self.assertRaises(owner.DisclosureError, owner.DisclosureLabel, **values)
        clock, boundary, reader = fixture()
        with reader:
            boundary.process = (True, 99, True)
            self.assertRaises(owner.DisclosureError, reader.read, 1000000)
        clock, boundary, reader = fixture()
        with reader:
            clock.read = lambda: True
            self.assertRaises(owner.DisclosureError, reader.read, 1000000)

    def test_windows_read_uses_one_marshaled_system_message_with_fixed_buffer_and_nonzero_bound(self):
        boundary = owner._WindowsBoundary.__new__(owner._WindowsBoundary)
        calls = []
        def send(hwnd, message, capacity, address, flags, timeout, result):
            calls.append((hwnd, message, capacity, flags, timeout))
            ctypes.memmove(address, ctypes.create_unicode_buffer(owner.LABELS[1]),
                           (len(owner.LABELS[1]) + 1) * ctypes.sizeof(ctypes.c_wchar))
            result._obj.value = len(owner.LABELS[1])
            return 1
        boundary.user = SimpleNamespace(SendMessageTimeoutW=send)
        with patch.object(ctypes, "set_last_error", create=True) as reset:
            result = boundary.text(12, 128, 7)
        reset.assert_called_once_with(0)
        self.assertEqual(calls, [(12, 0xD, 128, 0x23, 7)])
        self.assertEqual(result, (len(owner.LABELS[1]), owner.LABELS[1], True))
        boundary.user.SendMessageTimeoutW = lambda *_: 0
        with patch.object(ctypes, "set_last_error", create=True), patch.object(ctypes, "get_last_error", return_value=0, create=True):
            with self.assertRaisesRegex(owner.DisclosureError, "Windows error0|Windows error 0"):
                boundary.text(12, 128, 7)


if __name__ == "__main__":
    unittest.main()
