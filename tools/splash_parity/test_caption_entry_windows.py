# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Reject fabricated caption entry, foreign ownership and unobserved fixture teardown."""

import argparse
import itertools
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, Mock, patch

from tools.splash_parity.caption_entry_windows import caption_entry, drain_counts, observe


class CaptionEntryTests(unittest.TestCase):
    """Fake native boundaries without replacing entry/exit/deadline decisions."""

    def setUp(self) -> None:
        self.native = Mock()
        self.native.client.return_value = Mock(width=558, height=558, dpi=96)
        self.observer = Mock()
        self.observer.origin.return_value = (100, 100)
        self.observer.in_move_loop.side_effect = [False, False, True, True, False]
        self.process = Mock(pid=42)
        self.process.poll.return_value = None
        self.pointer = MagicMock()
        self.pointer.__enter__.return_value = self.pointer
        self.pointer.cursor.side_effect = lambda: (self.pointer.move_screen.call_args.args[0]
                                                  if self.pointer.move_screen.call_count else (350, 116))
        self.state = {}

    def run_probe(self):
        with patch("tools.splash_parity.caption_entry_windows.NativePointer", return_value=self.pointer), \
                patch("tools.splash_parity.caption_entry_windows.time.perf_counter", side_effect=itertools.count(0, .05)), \
                patch("tools.splash_parity.caption_entry_windows.time.sleep"):
            return caption_entry(self.native, self.observer, 1, self.process, self.state)

    def test_actual_motion_then_entry_release_and_exit_qualifies(self) -> None:
        result = self.run_probe()
        self.assertTrue(result["caption_entry_qualified"])
        self.assertFalse(result["physical_drag_qualified"])
        self.assertEqual(self.pointer.move.call_args_list[0].args, ((250, 16),))
        self.pointer.move.assert_called_once()
        self.pointer.move_screen.assert_called_once_with((354, 116))
        self.assertTrue(self.state["physical_movement_while_pressed"])
        self.assertTrue(self.state["entry_gesture"]["steps"][0]["input_dispatched"])
        self.pointer.press.assert_called_once()
        self.pointer.release.assert_called_once()
        self.assertEqual([row["gui_in_move_size_owned"] for row in self.state["entry_observations"]], [False, True])
        self.assertEqual([row["gui_in_move_size_owned"] for row in self.state["exit_observations"]], [True, False])

    def test_missing_entry_rejects_and_retains_pending_observations(self) -> None:
        self.observer.in_move_loop.side_effect = None
        self.observer.in_move_loop.return_value = False
        with self.assertRaisesRegex(RuntimeError, "enter.*within one second"):
            self.run_probe()
        self.assertFalse(self.state["entry_observations"][-1]["gui_in_move_size_owned"])
        self.assertIn("rejected_window_origin", self.state["entry_gesture"])
        self.assertLessEqual(self.pointer.move_screen.call_count, 16)
        self.pointer.__exit__.assert_called_once()

    def test_late_entry_cannot_qualify(self) -> None:
        self.observer.in_move_loop.side_effect = [False, True]
        with patch("tools.splash_parity.caption_entry_windows.NativePointer", return_value=self.pointer), \
                patch("tools.splash_parity.caption_entry_windows.time.perf_counter", side_effect=[0, 1.01]):
            with self.assertRaisesRegex(RuntimeError, "within one second"):
                caption_entry(self.native, self.observer, 1, self.process, self.state)
        self.assertTrue(self.state["entry_observations"][-1]["gui_in_move_size_owned"])
        self.assertNotIn("entry_observed_ms", self.state)

    def test_foreign_early_motion_rejects_and_retains_failed_gesture(self) -> None:
        self.pointer.move_screen.side_effect = RuntimeError("foreign physical movement target")
        with self.assertRaisesRegex(RuntimeError, "foreign physical"):
            self.run_probe()
        self.assertFalse(self.state["entry_gesture"]["steps"][0]["input_dispatched"])
        self.assertFalse(self.state["entry_observations"][0]["gui_in_move_size_owned"])
        self.pointer.__exit__.assert_called_once()

    def test_release_without_actual_exit_rejects(self) -> None:
        self.observer.in_move_loop.side_effect = itertools.chain([False], itertools.repeat(True))
        with self.assertRaisesRegex(RuntimeError, "did not exit"):
            self.run_probe()
        self.pointer.release.assert_called_once()
        self.assertTrue(self.state["exit_observations"][-1]["gui_in_move_size_owned"])

    def test_preexisting_move_loop_rejects_before_mouse_input(self) -> None:
        self.observer.in_move_loop.side_effect = None
        self.observer.in_move_loop.return_value = True
        with self.assertRaisesRegex(RuntimeError, "began with an active"):
            self.run_probe()
        self.pointer.__enter__.assert_not_called()

    def test_foreign_ownership_rejects_before_input(self) -> None:
        self.native.client.side_effect = RuntimeError("Foreign owned-window identity")
        with self.assertRaisesRegex(RuntimeError, "Foreign"):
            self.run_probe()
        self.pointer.__enter__.assert_not_called()

    def test_process_end_rejects_without_made_up_entry(self) -> None:
        self.process.poll.return_value = 0
        with self.assertRaisesRegex(RuntimeError, "ended before"):
            self.run_probe()
        self.assertEqual(self.state["entry_observations"], [])
        self.pointer.__exit__.assert_called_once()


class CaptionFixtureTests(unittest.TestCase):
    """Keep process output private and acceptance contingent on all owned teardown."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory(prefix="caption-entry-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.executable = self.root / "fixture.exe"
        self.executable.write_bytes(b"owned-fixture")
        self.arguments = argparse.Namespace(mode="rust", duration_ms=10000)
        self.native = Mock()
        self.native.find_window.return_value = 1
        self.fixture = Mock(receipt={"foreground_observed": True})
        self.process = Mock(pid=42, returncode=0)
        self.process.poll.return_value = 0
        self.process.communicate.return_value = (b"private-session-auth", b"private-diagnostic")

    def run_fixture(self, *, scenario_error=None, cleanup_error=None):
        self.fixture.__enter__ = Mock()
        self.fixture.__exit__ = Mock(side_effect=cleanup_error)
        with patch("tools.splash_parity.caption_entry_windows.launch", return_value=([str(self.executable)], self.root, {})), \
                patch("tools.splash_parity.caption_entry_windows.ForegroundLauncher", return_value=self.fixture), \
                patch("tools.splash_parity.caption_entry_windows.subprocess.Popen", return_value=self.process), \
                patch("tools.splash_parity.caption_entry_windows.observe_compositor", return_value=(None, 2., 1.)), \
                patch("tools.splash_parity.caption_entry_windows.ObservationWindows"), \
                patch("tools.splash_parity.caption_entry_windows.caption_entry", side_effect=scenario_error, return_value={"caption_entry_qualified": True}):
            return observe(self.arguments, self.root / "probe", self.native)

    def test_success_only_retains_output_counts(self) -> None:
        receipt = self.run_fixture()
        self.assertTrue(receipt["qualification_accepted"])
        self.assertFalse(receipt["process_output"][0]["content_retained"])
        self.assertEqual(receipt["process_output"][0]["stdout_bytes"], 20)
        for path in (self.root / "probe").iterdir():
            self.assertNotIn(b"private-session-auth", path.read_bytes())
            self.assertNotIn(b"private-diagnostic", path.read_bytes())
        self.fixture.__exit__.assert_called_once()

    def test_primary_and_cleanup_failure_both_preserved_without_success(self) -> None:
        with self.assertRaises(BaseExceptionGroup) as raised:
            self.run_fixture(scenario_error=RuntimeError("entry rejected"), cleanup_error=OSError("helper cleanup rejected"))
        self.assertEqual(len(raised.exception.exceptions), 2)
        failure = json.loads((self.root / "probe" / "failure.json").read_text())
        self.assertFalse(failure["qualification_accepted"])
        self.assertEqual([cause["error"] for cause in failure["failure_causes"]], ["entry rejected", "helper cleanup rejected"])
        self.assertFalse((self.root / "probe" / "caption-entry.json").exists())

    def test_successful_gesture_with_bad_process_exit_is_rejected(self) -> None:
        self.process.returncode = 1
        with self.assertRaisesRegex(RuntimeError, "status 1"):
            self.run_fixture()
        self.assertFalse((self.root / "probe" / "caption-entry.json").exists())

    def test_count_drain_rejects_incomplete_terminal_identity(self) -> None:
        self.process.returncode = None
        with self.assertRaisesRegex(RuntimeError, "terminal exit code"):
            drain_counts(self.process, {}, "test")

    def test_reference_child_is_bound_before_input_and_closed_on_failure(self) -> None:
        self.arguments.mode = "reference"
        for name in ("substitute/app/bootstrap/shared_splash_host.py", "substitute/presentation/shell/splash_window.py"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"read-only-production-source")
        owner = Mock(pid=77, metadata={"window_pid": 77})
        events = []
        owner.bind.side_effect = lambda deadline: events.append("verified-child")
        self.fixture.allow_product.side_effect = lambda pid: events.append(f"foreground-permission-{pid}")
        with patch("tools.splash_parity.caption_entry_windows.reference_base", return_value=self.executable), \
                patch("tools.splash_parity.caption_entry_windows.ReferenceProcesses"), \
                patch("tools.splash_parity.caption_entry_windows.ReferenceOwner", return_value=owner):
            with self.assertRaisesRegex(RuntimeError, "reference entry rejected"):
                self.run_fixture(scenario_error=RuntimeError("reference entry rejected"))
        self.assertEqual(events, ["verified-child", "foreground-permission-77"])
        self.native.find_window.assert_called_once()
        self.assertIs(self.native.find_window.call_args.args[0], owner)
        owner.close.assert_called_once()
        failure = json.loads((self.root / "probe" / "failure.json").read_text())
        self.assertEqual(failure["reference_process_owner"], {"window_pid": 77})
        self.assertEqual(failure["process_output"][0]["phase"], "owned_teardown")
        self.assertNotIn("private-session-auth", json.dumps(failure))


if __name__ == "__main__":
    unittest.main()
