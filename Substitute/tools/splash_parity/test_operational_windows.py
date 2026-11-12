# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Reject false native operational acceptance when input, pixels, ownership or load lifecycle fails."""

import ctypes
import hashlib
import itertools
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from ctypes import wintypes
from unittest.mock import MagicMock, Mock, patch

from PIL import Image

from tools.splash_parity.operational_windows import ExposureFailure, MonitorInfo, ObservationWindows, OperationalOutput, OwnedContention, affinity, distribution, main, static_source, terminal_frame_audit, terminal_move_audit
from tools.splash_parity.contention_worker import run as run_load_fixture


class DragObservationTests(unittest.TestCase):
    """Fake native boundaries while preserving movement, frame and deadline decisions."""

    def setUp(self) -> None:
        """Own portable screenshot fixtures in the caller's configured temporary root."""
        temporary = tempfile.TemporaryDirectory(prefix="physical-drag-")
        self.addCleanup(temporary.cleanup)
        self.output = Path(temporary.name)
        self.observer = object.__new__(ObservationWindows)
        self.observer.native = Mock()
        self.observer.native.client.return_value = Mock(width=558, height=558, scale=1)
        self.observer.native.capture.return_value = Image.new("RGB", (558, 558))
        self.source = bytes([0, 0, 0, 255]) * 558 * 558
        self.observer.native.user.GetSystemMetrics.return_value = 0
        self.observer.origin = Mock(side_effect=[(100, 100)] * 4 + [(108, 108), (116, 116)]
                                    + [(116 + index * 4, 116 + index * 2) for index in range(1, 9)] + [(148, 132)])
        self.observer.response = Mock(return_value=.1)

        def monitor(handle, pointer):
            info = ctypes.cast(pointer, ctypes.POINTER(MonitorInfo)).contents
            info.work.left, info.work.top, info.work.right, info.work.bottom = 0, 0, 1920, 1080
            return True

        self.observer.native.user.GetMonitorInfoW.side_effect = monitor
        self.pointer = MagicMock()
        self.pointer.__enter__.return_value = self.pointer
        self.pointer.cursor.side_effect = lambda: (self.pointer.move_screen.call_args.args[0]
                                                  if self.pointer.move_screen.call_count else (350, 116))
        self.observer.in_move_loop = Mock(side_effect=lambda hwnd, pid: self.pointer.move_screen.call_count >= 1
                                         and self.pointer.release.call_count == 0)

    def run_drag(self):
        """Use deterministic external time and a fake physical-input boundary."""
        with patch("tools.splash_parity.operational_windows.NativePointer", return_value=self.pointer), \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True), \
                patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=itertools.count(0, .01)), \
                patch("tools.splash_parity.operational_windows.time.sleep"):
            return self.observer.mouse_drag(1, 42, self.output, 8, expected_source=self.source)

    def test_real_movement_and_static_frames_produce_raw_distributions(self) -> None:
        """Qualification needs every physical step and observed static pixels."""
        result = self.run_drag()
        self.assertTrue(result["physical_mouse_drag_qualified"])
        self.assertIn("terminal native full-source audit", result["static_source_immutability"])
        self.assertTrue(result["initial_whole_frame_composite"]["accepted"])
        self.assertEqual(result["message_response"]["count"], 8)
        self.assertEqual(self.pointer.move_screen.call_count, 11)
        self.assertEqual(result["calibration"]["qualified_anchor_origin"], (116, 116))
        self.assertFalse(result["calibration"]["measured_distribution_member"])
        self.assertTrue(result["entry_gesture"]["steps"][0]["input_dispatched"])
        self.pointer.press.assert_called_once()
        self.pointer.release.assert_called_once()
        self.assertEqual(len(json.loads((self.output / "drag-samples.json").read_text())), 8)
        self.assertFalse(result["animated_source_activity_qualified"])

    def test_no_move_loop_is_not_physical_drag_acceptance(self) -> None:
        """A delivered button event without an OS move loop fails closed."""
        self.observer.in_move_loop.side_effect = None
        self.observer.in_move_loop.return_value = False
        self.observer.origin.side_effect = None
        self.observer.origin.return_value = (100, 100)
        with self.assertRaisesRegex(RuntimeError, "enter the owned OS move loop"):
            self.run_drag()
        self.assertLessEqual(self.pointer.move_screen.call_count, 16)
        self.pointer.__exit__.assert_called_once()
        rejected = json.loads((self.output / "drag-state.json").read_text())
        self.assertFalse(rejected["move_loop_observations"][0]["gui_in_move_size_owned"])
        self.assertIn("rejected_window_origin", rejected["entry_gesture"])

    def test_route_anchors_after_actual_owned_move_loop_entry(self) -> None:
        """Queued SC_MOVE establishes its real anchor before any physical route movement."""
        self.observer.origin.side_effect = [(100, 100), (100, 100), (105, 105), (107, 107), (115, 115), (123, 123)] + [(123 + index * 4, 123 + index * 2) for index in range(1, 9)] + [(155, 139)]

        def observe_move_state(hwnd, pid):
            return self.pointer.move_screen.call_count >= 1 and self.pointer.release.call_count == 0

        self.observer.in_move_loop.side_effect = observe_move_state
        result = self.run_drag()
        self.assertEqual(result["move_loop_entry"]["window_origin"], (107, 107))
        self.assertEqual(self.pointer.move_screen.call_args_list[0].args, ((354, 116),))
        self.assertEqual(self.pointer.move_screen.call_args_list[1].args, ((362, 124),))
        self.assertEqual(self.pointer.move_screen.call_args_list[2].args, ((370, 132),))
        self.assertEqual(self.pointer.move_screen.call_args_list[3].args, ((374, 134),))
        self.assertEqual(result["move_observation"]["count"], 8)

    def test_initial_physical_motion_causally_precedes_entry_and_route(self) -> None:
        events = []
        self.pointer.press.side_effect = lambda: events.append("press")
        self.pointer.move_screen.side_effect = lambda point: events.append("initial-motion" if self.pointer.move_screen.call_count == 1
                                                                          else "calibration-motion" if self.pointer.move_screen.call_count <= 3 else "route-motion")
        self.pointer.release.side_effect = lambda: events.append("release")

        def actual_entry(hwnd, pid):
            if self.pointer.release.call_count:
                events.append("observed-exit")
                return False
            if events == ["press"]:
                events.append("pending-entry")
                return False
            if events == ["press", "pending-entry", "initial-motion"]:
                events.append("observed-entry")
            return True

        self.observer.in_move_loop.side_effect = actual_entry
        self.run_drag()
        self.assertEqual(events, ["press", "pending-entry", "initial-motion", "observed-entry"] + ["calibration-motion"] * 2 + ["route-motion"] * 8 + ["release", "observed-exit"])

    def test_foreign_initial_motion_rejects_before_entry_and_route(self) -> None:
        self.pointer.move_screen.side_effect = RuntimeError("foreign early movement target")
        with self.assertRaisesRegex(RuntimeError, "foreign early"):
            self.run_drag()
        self.observer.in_move_loop.assert_called_once()
        self.pointer.__exit__.assert_called_once()
        state = json.loads((self.output / "drag-state.json").read_text())
        self.assertFalse(state["entry_gesture"]["steps"][0]["input_dispatched"])
        self.assertEqual(json.loads((self.output / "drag-samples.json").read_text()), [])

    def test_actual_entry_after_press_deadline_is_rejected(self) -> None:
        self.observer.in_move_loop.side_effect = [True]
        with patch("tools.splash_parity.operational_windows.NativePointer", return_value=self.pointer), \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True), \
                patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=[0, 1.01]):
            with self.assertRaisesRegex(RuntimeError, "within one second"):
                self.observer.mouse_drag(1, 42, self.output, 8, expected_source=self.source)
        state = json.loads((self.output / "drag-state.json").read_text())
        self.assertTrue(state["move_loop_observations"][-1]["gui_in_move_size_owned"])
        self.assertNotIn("move_loop_entry", state)
        self.pointer.move_screen.assert_not_called()

    def test_move_loop_loss_before_measurement_rejects_without_route_motion(self) -> None:
        self.observer.calibrate_drag = Mock(return_value=((116, 116), (370, 132)))
        self.observer.in_move_loop.side_effect = [False, True, False]
        with self.assertRaisesRegex(RuntimeError, "before a physical"):
            self.run_drag()
        self.assertEqual(self.pointer.move_screen.call_count, 1)
        state = json.loads((self.output / "drag-state.json").read_text())
        self.assertFalse(state["latest_movement"]["owned_move_loop_before_measurement"])
        self.assertEqual(json.loads((self.output / "drag-samples.json").read_text()), [])

    def test_move_loop_loss_during_measurement_rejects_captured_frame(self) -> None:
        self.observer.calibrate_drag = Mock(return_value=((116, 116), (370, 132)))
        self.observer.origin.side_effect = [(100, 100)] * 4 + [(120, 118)]
        self.observer.in_move_loop.side_effect = [False, True, True, False]
        with self.assertRaisesRegex(RuntimeError, "during a physical"):
            self.run_drag()
        state = json.loads((self.output / "drag-state.json").read_text())
        self.assertTrue(state["latest_movement"]["owned_move_loop_before_measurement"])
        self.assertFalse(state["latest_movement"]["owned_move_loop_after_measurement"])
        self.assertFalse(state["latest_movement"]["sample_accepted"])
        self.pointer.__exit__.assert_called_once()

    def test_work_area_admission_includes_the_maximum_horizontal_entry_motion(self) -> None:
        def small_work_area(handle, pointer):
            info = ctypes.cast(pointer, ctypes.POINTER(MonitorInfo)).contents
            info.work.left, info.work.top, info.work.right, info.work.bottom = 0, 0, 700, 1080
            return True

        self.observer.native.user.GetMonitorInfoW.side_effect = small_work_area
        with self.assertRaisesRegex(RuntimeError, "insufficient safe space"):
            self.run_drag()
        self.pointer.__enter__.assert_not_called()

    def test_work_area_admission_includes_both_diagonal_calibration_movements(self) -> None:
        def small_work_area(handle, pointer):
            info = ctypes.cast(pointer, ctypes.POINTER(MonitorInfo)).contents
            info.work.left, info.work.top, info.work.right, info.work.bottom = 0, 0, 706, 1080
            return True

        self.observer.origin.side_effect = None
        self.observer.origin.return_value = (40, 100)
        self.observer.native.user.GetMonitorInfoW.side_effect = small_work_area
        with self.assertRaisesRegex(RuntimeError, "insufficient safe space"):
            self.run_drag()
        self.pointer.__enter__.assert_not_called()

    def test_no_window_movement_is_rejected_and_samples_retained(self) -> None:
        """Cursor movement alone cannot satisfy a caption drag proof."""
        self.observer.origin.side_effect = None
        self.observer.origin.return_value = (100, 100)
        with self.assertRaisesRegex(RuntimeError, "calibration exceeded"):
            self.run_drag()
        self.assertEqual(json.loads((self.output / "drag-samples.json").read_text()), [])
        state = json.loads((self.output / "drag-state.json").read_text())
        self.assertEqual(state["calibration"]["steps"][0]["observations"][-1]["window_origin"], [100, 100])
        self.assertFalse(state["calibration"]["steps"][0]["accepted"])
        self.pointer.__exit__.assert_called_once()

    def test_mutated_static_frame_is_rejected_with_failure_image(self) -> None:
        """A raster change during reduced motion cannot be hidden by an origin-only pass."""
        self.observer.native.capture.side_effect = [Image.new("RGB", (558, 558)), Image.new("RGB", (558, 558), (1, 1, 1))]
        with self.assertRaisesRegex(RuntimeError, "source-over bounds"):
            self.run_drag()
        self.assertTrue((self.output / "drag-static-mismatch.png").exists())

    def test_unresponsive_message_boundary_is_rejected(self) -> None:
        """A timed-out WM_NULL never becomes a fabricated latency sample."""
        self.observer.response = ObservationWindows.response.__get__(self.observer)
        self.observer.native.user.SendMessageTimeoutW.return_value = 0
        with self.assertRaisesRegex(RuntimeError, "250 ms"):
            self.observer.response(1, 42)

    def test_actual_caption_without_visible_change_is_rejected(self) -> None:
        """Mouse insertion success must cause a compositor-observed disclosure change."""
        with patch("tools.splash_parity.operational_windows.NativePointer", return_value=self.pointer), \
                patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=itertools.count(0, .2)), \
                patch("tools.splash_parity.operational_windows.time.sleep"):
            with self.assertRaisesRegex(RuntimeError, "visibly respond"):
                self.observer.input_latency(1, 42, self.output, 8)
        self.assertEqual(json.loads((self.output / "input-latency-samples.json").read_text()), [])

    def test_actual_exposure_precedes_operational_pointer_scenario(self) -> None:
        """A PrintWindow mascot alone cannot begin a physical input distribution."""
        executable = self.output / "fixture.exe"
        executable.write_bytes(b"fixture")
        process = Mock(pid=42, returncode=0)
        process.poll.return_value = 0
        process.communicate.return_value = (b"", b"")
        observer = Mock()
        observer.input_latency.return_value = {}
        events = []
        observer.input_latency.side_effect = lambda *args: (events.append("gesture") or {})
        self.observer.native.exposure_state.return_value = {"foreground_owned": True, "occluding_windows": []}
        arguments = ["operational", "--executable", str(executable), "--output", str(self.output / "scenario"), "--mode", "input-latency"]
        with patch("sys.argv", arguments), \
                patch("tools.splash_parity.operational_windows.NativeWindows", return_value=self.observer.native), \
                patch("tools.splash_parity.operational_windows.ObservationWindows", return_value=observer), \
                patch("tools.splash_parity.operational_windows.subprocess.Popen", return_value=process) as launched, \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True), \
                patch("tools.splash_parity.operational_windows.observe_exposure", side_effect=lambda *args: (events.append("exposure") or 1)):
            self.assertEqual(main(), 0)
        self.assertEqual(events, ["exposure", "gesture"])
        self.assertEqual(launched.call_args.kwargs["creationflags"], 0x08000000)

    def test_failed_exposure_keeps_diagnostic_and_never_injects_input(self) -> None:
        """An obstructed window leaves a receipt before a pointer action can run."""
        executable = self.output / "fixture.exe"
        executable.write_bytes(b"fixture")
        process = Mock(pid=42, returncode=0)
        process.poll.return_value = 0
        process.communicate.return_value = (b"", b"owned native failure trace")
        observer = Mock()
        state = {"foreground_owned": False, "occluding_windows": [{"hwnd": 9, "pid": 99}]}
        arguments = ["operational", "--executable", str(executable), "--output", str(self.output / "blocked"), "--mode", "mouse-drag"]
        with patch("sys.argv", arguments), \
                patch("tools.splash_parity.operational_windows.NativeWindows", return_value=self.observer.native), \
                patch("tools.splash_parity.operational_windows.ObservationWindows", return_value=observer), \
                patch("tools.splash_parity.operational_windows.subprocess.Popen", return_value=process), \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True), \
                patch("tools.splash_parity.operational_windows.observe_exposure", side_effect=ExposureFailure(state)):
            with self.assertRaises(ExposureFailure):
                main()
        observer.mouse_drag.assert_not_called()
        failure = json.loads((self.output / "blocked" / "failure.json").read_text())
        self.assertEqual(failure["exposure_state"], state)
        self.assertFalse(failure["qualification_accepted"])
        self.assertTrue(failure["process_output"]["streams_complete"])
        self.assertEqual((self.output / "blocked" / "stderr-tail.log").read_bytes(), b"owned native failure trace")
        self.assertFalse((self.output / "blocked" / "observation.json").exists())

    def test_observation_and_owned_cleanup_failures_preserve_all_causes(self) -> None:
        """A rejected observation and failed termination/drain remain separately visible."""
        executable = self.output / "fixture.exe"
        executable.write_bytes(b"fixture")
        process = Mock(pid=42, returncode=None)
        process.poll.return_value = None
        process.terminate.side_effect = OSError("owned termination rejected")
        process.communicate.side_effect = subprocess.TimeoutExpired("owned fixture", 5, output=b"partial stdout", stderr=b"partial native trace")
        state = {"foreground_owned": False, "occluding_windows": []}
        destination = self.output / "cleanup-failure"
        arguments = ["operational", "--executable", str(executable), "--output", str(destination), "--mode", "mouse-drag"]
        with patch("sys.argv", arguments), \
                patch("tools.splash_parity.operational_windows.NativeWindows", return_value=self.observer.native), \
                patch("tools.splash_parity.operational_windows.ObservationWindows", return_value=Mock()), \
                patch("tools.splash_parity.operational_windows.subprocess.Popen", return_value=process), \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True), \
                patch("tools.splash_parity.operational_windows.observe_exposure", side_effect=ExposureFailure(state)):
            with self.assertRaises(ExceptionGroup):
                main()
        failure = json.loads((destination / "failure.json").read_text())
        self.assertEqual([cause["error_type"] for cause in failure["causes"]], ["ExposureFailure", "OSError", "TimeoutExpired"])
        self.assertFalse(failure["process_output"]["streams_complete"])
        self.assertTrue(failure["process_output"]["forced_termination_requested"])
        self.assertEqual((destination / "stderr-tail.log").read_bytes(), b"partial native trace")
        self.assertFalse((destination / "observation.json").exists())


class OperationalOutputTests(unittest.TestCase):
    """Exercise native diagnostic retention without touching authenticated reference streams."""

    def setUp(self) -> None:
        """Keep output tail artifacts in the caller's explicitly configured temporary root."""
        temporary = tempfile.TemporaryDirectory(prefix="owned-native-output-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.process = Mock(pid=42, returncode=0)
        self.process.poll.return_value = 0
        self.output = OperationalOutput(self.process, self.root)

    def test_graceful_close_retains_bounded_tails_complete_counts_and_exit(self) -> None:
        """Large diagnostic streams retain only their final 64 KiB while reporting actual counts."""
        stdout, stderr = b"a" * 70000 + b"stdout end", b"b" * 90000 + b"native trace end"
        self.process.communicate.return_value = stdout, stderr
        self.output.collect("graceful_close")
        receipt = json.loads((self.root / "process-output.json").read_text())
        self.assertEqual((self.root / "stdout-tail.log").read_bytes(), stdout[-65536:])
        self.assertEqual((self.root / "stderr-tail.log").read_bytes(), stderr[-65536:])
        self.assertEqual(receipt["stderr"]["bytes_observed"], len(stderr))
        self.assertTrue(receipt["stderr"]["tail_truncated"])
        self.assertTrue(receipt["streams_complete"])
        self.assertEqual(receipt["exit_code"], 0)
        self.assertEqual(receipt["phases"][0]["phase"], "graceful_close")
        self.process.terminate.assert_not_called()

    def test_failed_scenario_terminates_only_owned_process_and_observes_output(self) -> None:
        """A failed scenario retains its forced phase and terminal native trace."""
        self.process.poll.return_value = None
        self.process.returncode = -15
        self.process.communicate.return_value = b"", b"native failure cause"
        self.output.failed_teardown()
        self.process.terminate.assert_called_once()
        receipt = self.output.receipt()
        self.assertTrue(receipt["forced_termination_requested"])
        self.assertEqual(receipt["exit_code"], -15)
        self.assertEqual(receipt["phases"][-1]["phase"], "failed_scenario_teardown")

    def test_timed_out_drain_keeps_partial_counts_and_tail(self) -> None:
        """A timeout reports observed lower bounds without claiming complete output or shutdown."""
        self.process.returncode = None
        self.process.communicate.side_effect = subprocess.TimeoutExpired("owned", 5, output=b"x" * 70000, stderr=b"trace")
        with self.assertRaises(subprocess.TimeoutExpired):
            self.output.collect("graceful_close")
        self.output.persist()
        receipt = self.output.receipt()
        self.assertFalse(receipt["stdout"]["count_complete"])
        self.assertEqual(receipt["stdout"]["retained_tail_bytes"], 65536)
        self.assertIsNone(receipt["exit_code"])
        self.assertEqual(receipt["phases"][0]["causes"][0]["error_type"], "TimeoutExpired")

    def test_complete_streams_without_terminal_exit_cannot_pass(self) -> None:
        """Closed pipes alone do not prove the owned native process ended."""
        self.process.returncode = None
        self.process.communicate.return_value = b"", b"diagnostic"
        with self.assertRaisesRegex(RuntimeError, "terminal exit code"):
            self.output.collect("graceful_close")
        self.assertFalse(self.output.complete)

    def test_invalid_stream_type_cannot_mask_the_owned_drain_failure(self) -> None:
        """Malformed boundary output retains valid bytes without corrupting the failure receipt."""
        self.process.communicate.return_value = None, b"retained diagnostic"
        with self.assertRaisesRegex(RuntimeError, "complete byte streams"):
            self.output.collect("graceful_close")
        self.output.persist()
        self.assertEqual(self.output.receipt()["stderr"]["bytes_observed"], 19)
        self.assertFalse(self.output.complete)


class StaticFrameAuditTests(unittest.TestCase):
    """Require exact-build source provenance and complete native transition coverage."""

    def setUp(self) -> None:
        """Own raw frame and receipt fixtures under the configured temporary drive."""
        temporary = tempfile.TemporaryDirectory(prefix="full-frame-audit-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = bytes([70, 20, 30, 128])
        self.path = self.root / "first.raw"
        self.path.write_bytes(self.source)
        self.receipt = self.root / "submission.json"
        self.audit = {"width": 1, "height": 1, "presented_frames": 2,
                      "content_changes": 0, "byte_count": 4, "source_path": str(self.path)}

    def validate(self):
        """Exercise the terminal receipt reader after the owned process has ended."""
        self.receipt.write_text(json.dumps({"frames_presented": 2, "frame_audit": self.audit}))
        return terminal_frame_audit(self.receipt, self.path, self.source, 1, 1)

    def test_complete_native_audit_proves_whole_source_and_every_presentation(self) -> None:
        """Identical terminal bytes are coupled to a zero-change audit of both presented frames."""
        result = self.validate()
        self.assertTrue(result["complete_source_immutable"])
        self.assertEqual(result["source_sha256"], hashlib.sha256(self.source).hexdigest())

    def test_changed_then_restored_source_is_rejected(self) -> None:
        """A matching final/first source cannot conceal intervening source mutations."""
        self.audit["content_changes"] = 2
        with self.assertRaisesRegex(ValueError, "content changed"):
            self.validate()

    def test_missing_control_source_and_uncovered_presentations_are_rejected(self) -> None:
        """The complete source and native frame count each establish independent evidence."""
        self.path.write_bytes(bytes([69, 20, 30, 128]))
        with self.assertRaisesRegex(ValueError, "differs"):
            self.validate()
        self.path.write_bytes(self.source)
        self.audit["presented_frames"] = 1
        with self.assertRaisesRegex(ValueError, "every successfully presented"):
            self.validate()

    def test_foreign_path_invalid_geometry_and_unknown_audit_fields_fail_closed(self) -> None:
        """A source receipt cannot adopt arbitrary filesystem content or silently reinterpret metadata."""
        foreign = self.root / "foreign.raw"
        foreign.write_bytes(self.source)
        self.audit["source_path"] = str(foreign)
        with self.assertRaisesRegex(ValueError, "owned requested artifact"):
            self.validate()
        self.audit["source_path"] = str(self.path)
        self.audit["width"] = 2
        with self.assertRaisesRegex(ValueError, "geometry differs"):
            self.validate()
        self.audit["width"] = 1
        self.audit["unreviewed"] = True
        with self.assertRaisesRegex(ValueError, "schema"):
            self.validate()

    def test_exact_build_export_uses_observed_scale_and_raw_inactive_activity(self) -> None:
        """Preparation uses the production renderer without a second visible native window."""
        executable = self.root / "fixture.exe"
        executable.write_bytes(b"fixture")
        process = Mock(pid=47, returncode=0)
        process.poll.return_value = 0
        process.communicate.return_value = (b"", b"")

        def export(*args, **kwargs):
            (self.root / "static-source" / "static-first.raw").write_bytes(self.source)
            return process

        with patch("tools.splash_parity.operational_windows.subprocess.Popen", side_effect=export) as spawn:
            raw, provenance = static_source(executable, self.root, Mock(width=1, height=1, scale=1.75),
                                            hashlib.sha256(b"fixture").hexdigest())
        self.assertEqual(raw, self.source)
        definition = json.loads((self.root / "static-source" / "scenario.json").read_text())[0]
        self.assertEqual(definition["scale"], 1.75)
        self.assertFalse(definition["activity"])
        self.assertTrue(definition["raw"])
        self.assertEqual(provenance["export_pid"], 47)
        self.assertEqual(spawn.call_args.kwargs["env"]["TEMP"], str(self.root / "static-source" / "temporary"))

    def test_changed_executable_is_rejected_before_source_export(self) -> None:
        """The static source cannot silently come from a different binary than the launched window."""
        executable = self.root / "fixture.exe"
        executable.write_bytes(b"changed")
        with patch("tools.splash_parity.operational_windows.subprocess.Popen") as spawn:
            with self.assertRaisesRegex(RuntimeError, "changed before"):
                static_source(executable, self.root, Mock(), hashlib.sha256(b"fixture").hexdigest())
        spawn.assert_not_called()


class DragCalibrationTests(unittest.TestCase):
    """Require fresh diagonal native motion before adopting a measured cursor/window anchor."""

    def setUp(self) -> None:
        self.observer = object.__new__(ObservationWindows)
        self.observer.in_move_loop = Mock(return_value=True)
        self.observer.origin = Mock(side_effect=[(104, 100), (112, 108), (120, 116)])
        self.observer.response = Mock(return_value=.1)
        self.pointer = Mock()
        self.pointer.cursor.side_effect = lambda: self.pointer.move_screen.call_args.args[0]
        self.state = {}

    def calibrate(self, deadline=10):
        with patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=itertools.count(0, .01)), \
                patch("tools.splash_parity.operational_windows.time.sleep"):
            return self.observer.calibrate_drag(1, 42, self.pointer, (100, 100), (350, 116), (1, 1), deadline, self.state)

    def test_old_horizontal_input_cannot_seed_calibration_and_actual_diagonal_pair_qualifies(self) -> None:
        origin, cursor = self.calibrate()
        self.assertEqual(origin, (120, 116))
        self.assertEqual(cursor, (366, 132))
        self.assertEqual(self.state["steps"][0]["observations"][0]["window_origin"], (104, 100))
        self.assertEqual(self.state["steps"][0]["observed_window_origin"], (112, 108))
        self.assertEqual(self.state["steps"][1]["expected_window_origin"], (120, 116))
        self.assertTrue(all(step["accepted"] for step in self.state["steps"]))
        self.assertEqual(self.observer.in_move_loop.call_count, 4)

    def test_first_seed_late_vertical_response_is_rejected(self) -> None:
        self.observer.origin.side_effect = None
        self.observer.origin.return_value = (104, 100)
        with self.assertRaisesRegex(RuntimeError, "calibration exceeded"):
            self.calibrate()
        self.assertFalse(self.state["steps"][0]["accepted"])
        self.assertEqual(self.pointer.move_screen.call_count, 1)

    def test_second_point_mismatch_cannot_increase_position_tolerance(self) -> None:
        self.observer.origin.side_effect = itertools.chain([(112, 108)], itertools.repeat((124, 116)))
        with self.assertRaisesRegex(RuntimeError, "calibration exceeded"):
            self.calibrate()
        self.assertTrue(self.state["steps"][0]["accepted"])
        self.assertFalse(self.state["steps"][1]["accepted"])

    def test_loop_loss_during_calibration_rejects_paired_position(self) -> None:
        self.observer.in_move_loop.side_effect = [True, False]
        self.observer.origin.side_effect = [(112, 108)]
        with self.assertRaisesRegex(RuntimeError, "ended during"):
            self.calibrate()
        self.assertFalse(self.state["steps"][0]["accepted"])

    def test_foreign_motion_target_cannot_be_adopted(self) -> None:
        self.pointer.move_screen.side_effect = RuntimeError("foreign target")
        with self.assertRaisesRegex(RuntimeError, "foreign"):
            self.calibrate()
        self.assertFalse(self.state["steps"][0]["input_dispatched"])

    def test_actual_cursor_mismatch_rejects_despite_fresh_window_y(self) -> None:
        self.pointer.cursor.side_effect = None
        self.pointer.cursor.return_value = (350, 116)
        with self.assertRaisesRegex(RuntimeError, "cursor departed"):
            self.calibrate()

    def test_original_ten_second_deadline_is_not_extended_by_calibration(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "original ten-second"):
            self.calibrate(deadline=.025)
        self.assertFalse(self.state["steps"][0]["accepted"])

    def test_negative_direction_uses_the_same_admitted_route_and_seed_rule(self) -> None:
        self.observer.origin.side_effect = [(96, 100), (88, 92), (80, 84)]
        with patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=itertools.count(0, .01)), \
                patch("tools.splash_parity.operational_windows.time.sleep"):
            result = self.observer.calibrate_drag(1, 42, self.pointer, (100, 100), (350, 116), (-1, -1), 10, self.state)
        self.assertEqual(result, ((80, 84), (334, 100)))


class MovingSourceAuditTests(unittest.TestCase):
    """Distinguish native source transitions inside a real move loop from variable captured material."""

    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="moving-source-audit-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "first.raw"
        self.source.write_bytes(bytes([70, 20, 30, 128]))
        self.receipt = self.root / "submission.json"
        self.frame = {"width": 1, "height": 1, "presented_frames": 10, "content_changes": 5,
                      "byte_count": 4, "source_path": str(self.source)}
        self.audit = {"schema_version": 1, "move_frames_observed": 3, "move_samples_omitted": 0,
                      "presentations": [{"sequence": 3, "elapsed_ms": 100., "source_revision": 1},
                                        {"sequence": 4, "elapsed_ms": 116., "source_revision": 2},
                                        {"sequence": 5, "elapsed_ms": 148., "source_revision": 3}]}
        self.metadata = {"frames_presented": 10, "frame_audit": self.frame, "native_move_audit": self.audit,
                         "longest_frame_interval_ms": 850.}

    def validate(self):
        self.receipt.write_text(json.dumps(self.metadata), encoding="utf-8")
        return terminal_move_audit(self.receipt, self.source, 1, 1)

    def test_actual_in_loop_source_changes_report_distinct_submission_and_lifetime_intervals(self) -> None:
        result = self.validate()
        self.assertTrue(result["animated_source_activity_qualified"])
        self.assertEqual(result["whole_source_transitions_between_move_samples"], 2)
        self.assertEqual(result["in_loop_submission_intervals"]["p50_ms"], 16.)
        self.assertEqual(result["in_loop_submission_intervals"]["p95_ms"], 32.)
        self.assertEqual(result["in_loop_submission_intervals"]["p99_ms"], 32.)
        self.assertEqual(result["in_loop_sample_span_ms"], 48.)
        self.assertEqual(result["in_loop_first_submission_elapsed_ms"], 100.)
        self.assertEqual(result["in_loop_last_submission_elapsed_ms"], 148.)
        self.assertEqual(result["lifetime_longest_frame_interval_ms"], 850.)
        self.assertIn("frame age", result["timing_scope"])

    def test_global_source_changes_only_outside_loop_do_not_qualify(self) -> None:
        for sample in self.audit["presentations"]:
            sample["source_revision"] = 1
        with self.assertRaisesRegex(ValueError, "at least two consecutive"):
            self.validate()

    def test_multiple_changes_in_only_one_sample_pair_do_not_qualify(self) -> None:
        self.audit["presentations"][1]["source_revision"] = 3
        with self.assertRaisesRegex(ValueError, "at least two consecutive"):
            self.validate()

    def test_source_changes_in_intervening_outside_loop_presentations_cannot_qualify(self) -> None:
        self.audit["presentations"][1]["sequence"] = 6
        self.audit["presentations"][2]["sequence"] = 9
        with self.assertRaisesRegex(ValueError, "not consecutive lifetime"):
            self.validate()

    def test_missing_unknown_and_unsupported_move_schema_reject(self) -> None:
        for change in (lambda: self.metadata.pop("native_move_audit"),
                       lambda: self.audit.update(unreviewed=True),
                       lambda: self.audit.update(schema_version=2),
                       lambda: self.audit.update(schema_version=True)):
            with self.subTest(change=change):
                original = json.loads(json.dumps(self.metadata))
                change()
                with self.assertRaisesRegex(ValueError, "schema|version"):
                    self.validate()
                self.metadata = original
                self.audit = self.metadata["native_move_audit"]
                self.frame = self.metadata["frame_audit"]

    def test_omitted_miscounted_and_too_few_samples_reject(self) -> None:
        for field, value in (("move_samples_omitted", 1), ("move_frames_observed", 4),
                             ("move_frames_observed", True), ("move_samples_omitted", -1)):
            with self.subTest(field=field, value=value):
                prior = self.audit[field]
                self.audit[field] = value
                with self.assertRaises(ValueError):
                    self.validate()
                self.audit[field] = prior
        self.audit["presentations"] = self.audit["presentations"][:2]
        self.audit["move_frames_observed"] = 2
        with self.assertRaisesRegex(ValueError, "three through 512"):
            self.validate()

    def test_more_than512_samples_reject_instead_of_silently_truncating(self) -> None:
        self.audit["presentations"] = self.audit["presentations"] * 171
        self.audit["move_frames_observed"] = 513
        with self.assertRaisesRegex(ValueError, "three through 512"):
            self.validate()

    def test_sample_unknown_fields_nonfinite_and_invalid_integer_identities_reject(self) -> None:
        for field, value in (("elapsed_ms", float("nan")), ("elapsed_ms", float("inf")),
                             ("elapsed_ms", True), ("elapsed_ms", -1), ("sequence", 0),
                             ("sequence", 11), ("sequence", True), ("source_revision", -1),
                             ("source_revision", 6), ("source_revision", True), ("foreign_hwnd", 77)):
            with self.subTest(field=field, value=value):
                sample = self.audit["presentations"][0]
                previous = dict(sample)
                sample[field] = value
                with self.assertRaises(ValueError):
                    self.validate()
                self.audit["presentations"][0] = previous

    def test_reversed_or_equal_sequence_time_and_reversed_revision_reject(self) -> None:
        for field, value in (("sequence", 3), ("elapsed_ms", 100.), ("elapsed_ms", 99.), ("source_revision", 0)):
            with self.subTest(field=field):
                sample = self.audit["presentations"][1]
                prior = sample[field]
                sample[field] = value
                with self.assertRaisesRegex(ValueError, "not ordered"):
                    self.validate()
                sample[field] = prior

    def test_whole_frame_presentation_disagreement_and_invalid_transition_count_reject(self) -> None:
        self.metadata["frames_presented"] = 9
        with self.assertRaisesRegex(ValueError, "every successfully presented"):
            self.validate()
        self.metadata["frames_presented"] = 10
        self.frame["content_changes"] = 10
        with self.assertRaisesRegex(ValueError, "transition count"):
            self.validate()

    def test_unowned_truncated_or_invalid_premultiplied_first_source_reject(self) -> None:
        foreign = self.root / "foreign.raw"
        foreign.write_bytes(bytes(4))
        self.frame["source_path"] = str(foreign)
        with self.assertRaisesRegex(ValueError, "owned requested"):
            self.validate()
        self.frame["source_path"] = str(self.source)
        self.source.write_bytes(bytes(3))
        with self.assertRaisesRegex(ValueError, "byte count"):
            self.validate()
        self.source.write_bytes(bytes([129, 20, 30, 128]))
        with self.assertRaises(ValueError):
            self.validate()

    def test_animated_main_requires_terminal_audit_and_retains_full_first_source_path(self) -> None:
        executable = self.root / "fixture.exe"
        executable.write_bytes(b"exact-native-build")
        process = Mock(pid=42, returncode=0)
        process.poll.return_value = 0
        process.communicate.return_value = (b"", b"")
        native, observer = Mock(), Mock()
        native.client.return_value = Mock(width=558, height=558, scale=1)
        native.exposure_state.return_value = {"foreground_owned": True, "occluding_windows": []}
        observer.mouse_drag.return_value = {"animated_source_activity_qualified": False}
        output = self.root / "animated"
        arguments = ["operational", "--executable", str(executable), "--output", str(output),
                     "--mode", "mouse-drag", "--animated"]
        with patch("sys.argv", arguments), \
                patch("tools.splash_parity.operational_windows.NativeWindows", return_value=native), \
                patch("tools.splash_parity.operational_windows.ObservationWindows", return_value=observer), \
                patch("tools.splash_parity.operational_windows.subprocess.Popen", return_value=process) as spawn, \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True), \
                patch("tools.splash_parity.operational_windows.observe_exposure", return_value=1.), \
                patch("tools.splash_parity.operational_windows.terminal_move_audit", return_value={"animated_source_activity_qualified": True}) as validator:
            self.assertEqual(main(), 0)
        command = spawn.call_args.args[0]
        self.assertEqual(command[command.index("--frame-audit") + 1], str(output / "audit-first.raw"))
        self.assertNotIn("--reduced-motion", command)
        validator.assert_called_once_with(output / "submission.json", output / "audit-first.raw", 558, 558)
        result = json.loads((output / "observation.json").read_text())
        self.assertTrue(result["animated_source_activity_qualified"])

    def test_failed_terminal_audit_prevents_any_successful_animated_receipt(self) -> None:
        executable = self.root / "fixture.exe"
        executable.write_bytes(b"exact-native-build")
        process = Mock(pid=42, returncode=0)
        process.poll.return_value = 0
        process.communicate.return_value = (b"", b"native-proof-diagnostic")
        native, observer = Mock(), Mock()
        native.client.return_value = Mock(width=558, height=558, scale=1)
        native.exposure_state.return_value = {"foreground_owned": True, "occluding_windows": []}
        observer.mouse_drag.return_value = {"animated_source_activity_qualified": False}
        output = self.root / "animated-rejected"
        arguments = ["operational", "--executable", str(executable), "--output", str(output),
                     "--mode", "mouse-drag", "--animated"]
        with patch("sys.argv", arguments), \
                patch("tools.splash_parity.operational_windows.NativeWindows", return_value=native), \
                patch("tools.splash_parity.operational_windows.ObservationWindows", return_value=observer), \
                patch("tools.splash_parity.operational_windows.subprocess.Popen", return_value=process), \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True), \
                patch("tools.splash_parity.operational_windows.observe_exposure", return_value=1.), \
                patch("tools.splash_parity.operational_windows.terminal_move_audit", side_effect=ValueError("outside-only source changes")):
            with self.assertRaisesRegex(ValueError, "outside-only"):
                main()
        self.assertFalse((output / "observation.json").exists())
        failure = json.loads((output / "failure.json").read_text())
        self.assertFalse(failure["qualification_accepted"])
        self.assertIn("outside-only", failure["causes"][0]["error"])


class ContentionOwnershipTests(unittest.TestCase):
    """Exercise load handshake, process identity and bounded cleanup without launching load."""

    def load(self) -> OwnedContention:
        """Supply explicit process snapshots and retained terminal outcomes."""
        observer = Mock()
        observer.kernel.OpenProcess.return_value = 7
        observer.kernel.CloseHandle.return_value = True
        observer.snapshot.side_effect = [{"creation_ticks": 1, "cpu_100ns": 0},
                                         {"creation_ticks": 1, "cpu_100ns": 1_000_000}]
        return OwnedContention(observer, 1, 3)

    def process(self) -> Mock:
        """Keep owned subprocess termination isolated from any PID-based process operation."""
        process = Mock(pid=42, returncode=-15)
        process.poll.return_value = None
        process.communicate.return_value = (b"", b"")
        return process

    def test_foreign_or_incomplete_readiness_is_rejected(self) -> None:
        """PID, nonce, affinity and lifetime all participate in readiness admission."""
        load = self.load()
        expected = {"pid": 42, "token": "a" * 32, "affinity_mask": 3, "maximum_lifetime_seconds": 15}
        self.assertEqual(load.validate_ready(json.dumps(expected).encode(), 42, "a" * 32), expected)
        for key, bad in (("pid", 43), ("token", "b" * 32), ("affinity_mask", 1), ("maximum_lifetime_seconds", 30)):
            with self.subTest(field=key):
                with self.assertRaisesRegex(RuntimeError, "identity or affinity"):
                    load.validate_ready(json.dumps({**expected, key: bad}).encode(), 42, "a" * 32)

    def test_failed_ready_reaps_the_already_launched_worker(self) -> None:
        """Readiness failure does not orphan a worker or leak its observation handle."""
        load, process = self.load(), self.process()
        load.ready = Mock(side_effect=RuntimeError("ready fixture timeout"))
        with patch("tools.splash_parity.operational_windows.subprocess.Popen", return_value=process) as launched, \
                patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=[0, 1]):
            with self.assertRaisesRegex(RuntimeError, "ready fixture timeout"):
                load.__enter__()
        self.assertEqual(launched.call_args.args[0][0], str(Path(sys._base_executable).resolve(strict=True)))
        self.assertEqual(Path(launched.call_args.args[0][1]).name, "contention_worker.py")
        self.assertEqual(launched.call_args.kwargs["creationflags"], 0x08000000)
        process.terminate.assert_called_once()
        process.communicate.assert_called_once_with(timeout=2)
        load.observer.kernel.CloseHandle.assert_called_once_with(7)

    def test_stdlib_worker_reports_actual_affinity_before_its_bounded_lifetime(self) -> None:
        """An external affinity acknowledgement is recorded; the fake deadline prevents real load."""
        kernel = Mock()
        kernel.SetProcessAffinityMask.return_value = True

        def observe(handle, current, system):
            ctypes.cast(current, ctypes.POINTER(ctypes.c_size_t)).contents.value = 3
            ctypes.cast(system, ctypes.POINTER(ctypes.c_size_t)).contents.value = 15
            return True

        kernel.GetProcessAffinityMask.side_effect = observe
        with patch("tools.splash_parity.contention_worker.ctypes.WinDLL", return_value=kernel), \
                patch("tools.splash_parity.contention_worker.time.perf_counter", side_effect=[0, 16]), \
                patch("tools.splash_parity.contention_worker.os.getpid", return_value=42), \
                patch("builtins.print") as printed:
            run_load_fixture("a" * 32, 3)
        self.assertEqual(json.loads(printed.call_args.args[0]),
                         {"pid": 42, "token": "a" * 32, "affinity_mask": 3, "maximum_lifetime_seconds": 15})

    def test_invalid_worker_identity_cannot_set_affinity_or_start_cpu_work(self) -> None:
        """The worker rejects malformed admission before touching any process state."""
        with patch("tools.splash_parity.contention_worker.ctypes.WinDLL") as native:
            with self.assertRaises(ValueError):
                run_load_fixture("invalid", 3)
            with self.assertRaises(ValueError):
                run_load_fixture("a" * 32, 0)
        native.assert_not_called()

    def test_cleanup_rejects_identity_change_but_still_reaps_owned_handle(self) -> None:
        """Counter identity mismatch must not excuse leaving the owned load process alive."""
        load, process = self.load(), self.process()
        load.observer.snapshot.side_effect = None
        load.observer.snapshot.return_value = {"creation_ticks": 2, "cpu_100ns": 1_000_000}
        load.entries = [{"process": process, "handle": 7, "before": {"creation_ticks": 1, "cpu_100ns": 0}, "started": 0}]
        with self.assertRaises(ExceptionGroup) as caught:
            load.__exit__(None, None, None)
        self.assertIn("identity changed", str(caught.exception.exceptions[0]))
        process.terminate.assert_called_once()
        load.observer.kernel.CloseHandle.assert_called_once_with(7)

    def test_worker_early_exit_is_not_contended_acceptance(self) -> None:
        """A completed scenario without live load is explicitly rejected."""
        load, process = self.load(), self.process()
        process.poll.return_value = 0
        load.observer.snapshot.side_effect = None
        load.observer.snapshot.return_value = {"creation_ticks": 1, "cpu_100ns": 1_000_000}
        load.entries = [{"process": process, "handle": 7, "before": {"creation_ticks": 1, "cpu_100ns": 0}, "started": 0}]
        with patch("tools.splash_parity.operational_windows.time.perf_counter", return_value=1):
            with self.assertRaises(ExceptionGroup) as caught:
                load.__exit__(None, None, None)
        self.assertIn("ended before", str(caught.exception.exceptions[0]))
        process.terminate.assert_not_called()
        process.communicate.assert_called_once_with(timeout=2)

    def test_invalid_affinity_and_invalid_distribution_reject(self) -> None:
        """Do not infer CPUs or invent latency samples for an empty receipt."""
        for value in ("0,0", "-1", "64"):
            with self.assertRaises(ValueError):
                affinity(value)
        for values in ([], [-1], [float("inf")]):
            with self.assertRaises(ValueError):
                distribution(values)
        self.assertEqual(distribution(list(range(1, 101)))["p99_ms"], 99)

    def test_missing_oversized_and_late_ready_are_bounded_failures(self) -> None:
        """Ready pipes cannot block indefinitely or admit an unchecked payload."""
        for size, clocks, message in ((0, [0, 0, 2], "within one second"),
                                      (4097, [0, 0], "payload limit"),
                                      (2, [0, 0, 2], "after its live")):
            with self.subTest(payload_size=size):
                load, process = self.load(), self.process()
                process.stdout.read.return_value = b"{}\n"

                def peek(handle, buffer, length, read, available, remaining):
                    ctypes.cast(available, ctypes.POINTER(wintypes.DWORD)).contents.value = size
                    return True

                load.observer.kernel.PeekNamedPipe.side_effect = peek
                with patch("msvcrt.get_osfhandle", return_value=7), \
                        patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=clocks), \
                        patch("tools.splash_parity.operational_windows.time.sleep"):
                    with self.assertRaisesRegex(RuntimeError, message):
                        load.ready(process, "a" * 32)

    def test_terminal_timeout_retains_cause_and_releases_observation_handle(self) -> None:
        """A timed-out child shutdown remains a failure despite successful handle cleanup."""
        load, process = self.load(), self.process()
        load.observer.snapshot.side_effect = None
        load.observer.snapshot.return_value = {"creation_ticks": 1, "cpu_100ns": 1_000_000}
        load.entries = [{"process": process, "handle": 7, "before": {"creation_ticks": 1, "cpu_100ns": 0}, "started": 0}]
        process.communicate.side_effect = subprocess.TimeoutExpired("owned fixture", 2)
        with patch("tools.splash_parity.operational_windows.time.perf_counter", return_value=1):
            with self.assertRaises(ExceptionGroup) as caught:
                load.__exit__(None, None, None)
        self.assertIsInstance(caught.exception.exceptions[0], subprocess.TimeoutExpired)
        process.terminate.assert_called_once()
        load.observer.kernel.CloseHandle.assert_called_once_with(7)


class CpuObservationTests(unittest.TestCase):
    """Fake the external process clock and native boundaries, exercising owned observation cleanup."""

    def observer(self, *, cpu_ticks: int = 1_000_000, creation_ticks: int = 1) -> ObservationWindows:
        """Supply isolated OS snapshots without opening a real process or window."""
        observer = object.__new__(ObservationWindows)
        observer.native = Mock()
        observer.native.client.return_value.scale = 1
        observer.kernel = Mock()
        observer.kernel.OpenProcess.return_value = 7
        observer.kernel.WaitForSingleObject.return_value = 0x102
        observer.kernel.CloseHandle.return_value = True
        observer.snapshot = Mock(side_effect=[
            {"creation_ticks": 1, "cpu_100ns": 0},
            {"creation_ticks": creation_ticks, "cpu_100ns": cpu_ticks},
        ])
        return observer

    def test_one_core_percentage_and_restoration_use_measured_cpu(self) -> None:
        """A 100 ms CPU interval over 30 seconds is one third of one core percent."""
        observer = self.observer()
        with patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=[0, 30, 30]), \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True):
            result = observer.cpu(1, 42, "visible")
        self.assertAlmostEqual(result["one_logical_core_percent"], 1 / 3)
        self.assertTrue(result["below_one_percent"])
        self.assertTrue(result["restore_paint_observed"])
        observer.kernel.WaitForSingleObject.assert_called_once_with(7, 30000)
        observer.kernel.CloseHandle.assert_called_once_with(7)

    def test_cpu_over_budget_is_an_explicit_failure_decision(self) -> None:
        """A noisy static process is not accepted merely because its 30-second wait completes."""
        observer = self.observer(cpu_ticks=6_000_000)
        with patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=[0, 30, 30]), \
                patch("tools.splash_parity.operational_windows.has_mascot", return_value=True):
            self.assertFalse(observer.cpu(1, 42, "visible")["below_one_percent"])

    def test_early_exit_and_reused_identity_fail_and_release_handle(self) -> None:
        """Neither early process loss nor mixed-identity counters can satisfy an idle receipt."""
        for reuse in (False, True):
            with self.subTest(reused_identity=reuse):
                observer = self.observer(creation_ticks=2 if reuse else 1)
                if not reuse:
                    observer.kernel.WaitForSingleObject.return_value = 0
                with patch("tools.splash_parity.operational_windows.time.perf_counter", side_effect=[0, 30]):
                    with self.assertRaises(RuntimeError):
                        observer.cpu(1, 42, "visible")
                observer.kernel.CloseHandle.assert_called_once_with(7)

    def test_failed_hide_is_rejected_before_measurement(self) -> None:
        """The requested hidden interval must actually be hidden."""
        observer = self.observer()
        observer.native.user.IsWindowVisible.return_value = True
        with self.assertRaisesRegex(RuntimeError, "hidden"):
            observer.cpu(1, 42, "hidden")
        observer.kernel.WaitForSingleObject.assert_not_called()
        observer.kernel.CloseHandle.assert_called_once_with(7)


if __name__ == "__main__":
    unittest.main()
