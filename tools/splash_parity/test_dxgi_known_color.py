# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Test palette, freshness, inventory and terminal metadata without native GUI."""
import json
import unittest
import tempfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
from dataclasses import replace
from tools.splash_parity import dxgi_known_color as owner
from tools.splash_parity.test_m0_windows import Clock, Capture


class KnownColorTests(unittest.TestCase):
    def samples(self):
        clock = Clock()
        capture = Capture(clock)
        first = replace(capture.sample(deadline_ticks=1000000), bgra=owner.palette_bgra(0))
        second = replace(capture.sample(deadline_ticks=1000000), bgra=owner.palette_bgra(1))
        return clock, first, second

    def paints(self, first, second):
        records = []
        for index, sample in enumerate((first, second)):
            records.append(dict(event="painted", pid=55, hwnd=11, thread=71, revision=index + 1,
                                paint_qpc=sample.begin_ticks, qpc_frequency=1000000, button_id=1001,
                                roi=dict(x=sample.geometry.screen_x + 40, y=sample.geometry.screen_y + 40, width=2, height=2),
                                bgra=[list(owner.palette_bgra(index)[offset:offset + 4]) for offset in range(0, 16, 4)]))
        return records

    def verify(self, raw, first, second):
        return owner.verify_paints(raw, 55, 11, 71, 1000000, first, second, second.begin_ticks)

    def test_asymmetric_four_pixels_prove_channels_orientation_and_second_palette(self):
        first, second = owner.palette_bgra(0), owner.palette_bgra(1)
        self.assertTrue(owner.color_equal(first, first))
        self.assertFalse(owner.color_equal(first, second))
        self.assertFalse(owner.color_equal(first, first[8:] + first[:8]))
        swapped = bytes(value for pixel in owner.PALETTES[0] for value in (*pixel, 255))
        self.assertFalse(owner.color_equal(swapped, first))
        self.assertFalse(owner.color_equal(first[:4] * 4, first))
        alpha_only = bytes(value if index % 4 != 3 else 0 for index, value in enumerate(first))
        self.assertTrue(owner.color_equal(alpha_only, first))

    def test_fixture_terminal_requires_two_exact_owned_revisions_and_qpc(self):
        clock, first, second = self.samples()
        records = self.paints(first, second)
        self.assertEqual(self.verify("\n".join(json.dumps(r) for r in records).encode(), first, second), records)
        changes = [lambda r: r[1].update(pid=56), lambda r: r[1].update(thread=72),
                   lambda r: r[1].update(revision=1), lambda r: r[1].update(qpc_frequency=1),
                   lambda r: r[1].update(paint_qpc=second.begin_ticks - 1),
                   lambda r: r[1].update(paint_qpc=second.end_ticks + 1),
                   lambda r: r[1].update(button_id=True), lambda r: r[1].update(bgra=r[0]["bgra"]),
                   lambda r: r[1]["roi"].update(x=141), lambda r: r[1].update(extra="unknown")]
        for change in changes:
            modified = self.paints(first, second)
            change(modified)
            self.assertRaises((RuntimeError, ValueError), self.verify,
                              "\n".join(json.dumps(r) for r in modified).encode(), first, second)
        for raw in (b"x" * 4097, b"", json.dumps(records[0]).encode(), b"\xff"):
            self.assertRaises((RuntimeError, UnicodeError), self.verify, raw, first, second)
        duplicate = (json.dumps(records[0]) + "\n" + json.dumps(records[1]).replace('"revision": 2', '"revision": 1, "revision": 2')).encode()
        self.assertRaisesRegex(ValueError, "Duplicate JSON key", self.verify, duplicate, first, second)

    def test_matching_palette_never_qualifies_after_original_deadline_or_without_real_color(self):
        clock, first, second = self.samples()
        class External:
            def sample(self, **options):
                return first
        self.assertRaises(RuntimeError, owner.observe_palette, External(), clock, 0, 0, first.end_ticks, [])
        records = []
        self.assertRaises(RuntimeError, owner.observe_palette, External(), clock, 1, 0, 1000000, records)
        self.assertEqual(len(records), 256)

    def test_bad_fixture_hash_precedes_native_loader_and_product_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            dll, fixture = Path(directory) / "helper.dll", Path(directory) / "fixture.exe"
            dll.write_bytes(b"exact DLL fixture")
            fixture.write_bytes(b"exact native fixture")
            args = SimpleNamespace(capture_helper=dll, capture_helper_sha256=owner.hashlib.sha256(dll.read_bytes()).hexdigest(),
                                   fixture=fixture, fixture_sha256="0" * 64, output=Path(directory) / "out")
            with patch.object(owner, "DxgiLibrary") as load, patch.object(owner.subprocess, "Popen") as launch:
                self.assertRaises(ValueError, owner.run, args)
            load.assert_not_called()
            launch.assert_not_called()

    def test_initial_foreground_failure_preserves_cause_and_owned_launcher_cleanup_without_launch(self):
        events = []
        class Launcher:
            receipt = {"actual_gesture": "failed foreground condition"}
            def __init__(self, native):
                pass
            def __enter__(self):
                events.append("activation")
                raise RuntimeError("owned activation failure")
            def __exit__(self, *_):
                events.append("launcher_cleanup")
        with tempfile.TemporaryDirectory() as directory:
            dll, fixture = Path(directory) / "helper.dll", Path(directory) / "fixture.exe"
            dll.write_bytes(b"exact DLL fixture")
            fixture.write_bytes(b"exact native fixture")
            args = SimpleNamespace(capture_helper=dll, capture_helper_sha256=owner.hashlib.sha256(dll.read_bytes()).hexdigest(),
                                   fixture=fixture, fixture_sha256=owner.hashlib.sha256(fixture.read_bytes()).hexdigest(), output=Path(directory) / "out")
            with patch.object(owner, "DxgiLibrary", return_value=object()), patch.object(owner, "NativeWindows", return_value=object()), \
                    patch.object(owner, "ReferenceProcesses", return_value=object()), patch.object(owner, "Qpc", Clock), \
                    patch.object(owner, "ForegroundLauncher", Launcher), patch.object(owner.subprocess, "Popen") as launch:
                self.assertRaisesRegex(RuntimeError, "owned activation failure", owner.run, args)
            launch.assert_not_called()
            saved = json.loads((args.output / "qualification.json").read_text())
            self.assertFalse(saved["accepted"])
            parser_path = Path(owner.__file__).with_name("native_timing.py")
            self.assertEqual(saved["harness_sha256"]["native_timing.py"], owner.hashlib.sha256(parser_path.read_bytes()).hexdigest())
            self.assertIn("owned activation failure", str(saved["failure_causes"]))
            self.assertEqual(events, ["activation", "launcher_cleanup"])

    def fixture_admission(self, *, find_error=None, window_pid=55, thread=71, changed_process=False,
                          elapsed=False, permission_error=False):
        events, deadlines = [], []
        class Process:
            pid, _handle, returncode = 55, 77, None
            def poll(self):
                return self.returncode
        process = Process()
        class Native:
            def __init__(self):
                self.user = self
            def find_window(self, actual, deadline):
                self_test.assertIs(actual, process)
                events.append("find_window")
                deadlines.append(deadline)
                if find_error:
                    raise RuntimeError(find_error)
                return 11
            def GetWindowThreadProcessId(self, hwnd, target):
                events.append("window_thread")
                target._obj.value = window_pid
                return thread
            def owns(self, hwnd, pid):
                return hwnd == 11 and pid == window_pid
            def IsWindowVisible(self, hwnd):
                return hwnd == 11
            def post(self, *args):
                events.append("graceful_close")
        class Launcher:
            receipt = {}
            def __init__(self, native):
                pass
            def __enter__(self):
                events.append("activation")
            def allow_product(self, pid):
                self_test.assertEqual(pid, 55)
                self_test.assertIn("find_window", events)
                self_test.assertIn("window_thread", events)
                events.append("allow")
                if permission_error:
                    raise RuntimeError("permission native failure")
            def __exit__(self, *_):
                events.append("launcher_cleanup")
        class Output:
            complete, stdout, stderr = False, b"", b""
            def __init__(self, actual, output):
                self_test.assertIs(actual, process)
            def collect(self, phase):
                events.append(phase)
                self.complete, process.returncode = True, 0
            def failed_teardown(self):
                events.append("forced_teardown")
                self.complete, process.returncode = True, 1
            def receipt(self):
                return dict(exit_code=process.returncode, complete=self.complete)
        self_test = self
        with tempfile.TemporaryDirectory() as directory:
            dll, fixture = Path(directory) / "helper.dll", Path(directory) / "fixture.exe"
            dll.write_bytes(b"exact DLL fixture")
            fixture.write_bytes(b"exact native fixture")
            args = SimpleNamespace(capture_helper=dll, capture_helper_sha256=owner.hashlib.sha256(dll.read_bytes()).hexdigest(),
                                   fixture=fixture, fixture_sha256=owner.hashlib.sha256(fixture.read_bytes()).hexdigest(), output=Path(directory) / "out")
            identity_calls = []
            def identity(handle):
                self.assertEqual(handle, 77)
                identity_calls.append(handle)
                events.append("process_identity")
                return (55, fixture, 101 if changed_process and len(identity_calls) > 1 else 100)
            processes = SimpleNamespace(identity=identity, alive=lambda handle: handle == 77)
            def exposure(native, hwnd, actual, deadline):
                events.append("exposure")
                deadlines.append(deadline)
                raise RuntimeError("observed exposure failure")
            ticks = iter((10, 15 if elapsed is True else 11, 15 if elapsed == "admission" else 12))
            with patch.object(owner, "DxgiLibrary", return_value=object()), patch.object(owner, "NativeWindows", Native), \
                    patch.object(owner, "ReferenceProcesses", return_value=processes), patch.object(owner, "Qpc", Clock), \
                    patch.object(owner, "ForegroundLauncher", Launcher), patch.object(owner, "OperationalOutput", Output), \
                    patch.object(owner.subprocess, "Popen", return_value=process), patch.object(owner, "observe_exposure", exposure), \
                    patch.object(owner.time, "perf_counter", side_effect=lambda: next(ticks)), patch.object(owner, "RegionCapture") as capture:
                with self.assertRaises(RuntimeError) as failure:
                    owner.run(args)
                capture.assert_not_called()
            saved = json.loads((args.output / "qualification.json").read_text())
            return str(failure.exception), saved, events, deadlines

    def test_fixture_permission_follows_owned_visible_window_thread_and_unchanged_deadline(self):
        error, saved, events, deadlines = self.fixture_admission()
        self.assertEqual(error, "observed exposure failure")
        self.assertLess(events.index("find_window"), events.index("window_thread"))
        self.assertLess(events.index("window_thread"), events.index("allow"))
        self.assertLess(events.index("allow"), events.index("exposure"))
        self.assertEqual(deadlines, [15, 15])
        self.assertEqual(saved["window_identity"], dict(pid=55, ui_thread_id=71))
        self.assertEqual(saved["foreground_permission"], dict(pid=55, attempted=True, completed=True))
        self.assertIsNone(saved["process_before_cleanup"]["exit_code"])
        self.assertEqual(saved["process_output"]["exit_code"], 0)

    def test_foreign_or_invalid_window_thread_and_changed_process_reject_before_permission(self):
        for options in (dict(window_pid=56), dict(thread=0), dict(thread=-1), dict(thread=True), dict(changed_process=True)):
            with self.subTest(options=options):
                _, saved, events, _ = self.fixture_admission(**options)
                self.assertNotIn("allow", events)
                self.assertNotIn("exposure", events)
                self.assertFalse(saved["accepted"])

    def test_no_window_or_late_window_never_grants_permission_and_retains_actual_cleanup(self):
        for options in (dict(find_error="native window enumeration failure"), dict(elapsed=True)):
            with self.subTest(options=options):
                error, saved, events, _ = self.fixture_admission(**options)
                self.assertNotIn("allow", events)
                self.assertNotIn("window_thread", events)
                self.assertIn(error, str(saved["failure_causes"]))
                self.assertIsNone(saved["process_before_cleanup"]["exit_code"])
                self.assertTrue(saved["process_output"]["complete"])
                self.assertIn("launcher_cleanup", events)
                if "find_error" in options:
                    self.assertIn("forced_teardown", events)
                    self.assertEqual(saved["process_output"]["exit_code"], 1)
                    self.assertNotIn("hwnd", saved)

    def test_identity_checks_cannot_reset_an_expired_original_admission_deadline(self):
        error, saved, events, deadlines = self.fixture_admission(elapsed="admission")
        self.assertIn("original launch deadline", error)
        self.assertIn("window_thread", events)
        self.assertNotIn("allow", events)
        self.assertNotIn("exposure", events)
        self.assertEqual(deadlines, [15])
        self.assertEqual(saved["launch_deadline_seconds"], 15)

    def test_permission_failure_preserves_observed_native_effects_before_owned_cleanup(self):
        error, saved, events, _ = self.fixture_admission(permission_error=True)
        self.assertEqual(error, "permission native failure")
        self.assertEqual(saved["hwnd"], 11)
        self.assertEqual(saved["window_identity"], dict(pid=55, ui_thread_id=71))
        self.assertEqual(saved["foreground_permission"], dict(pid=55, attempted=True, completed=False))
        self.assertNotIn("exposure", events)
        self.assertIsNone(saved["process_before_cleanup"]["exit_code"])
        self.assertEqual(saved["process_output"]["exit_code"], 0)


if __name__ == "__main__":
    unittest.main()
