# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Reject false or unbounded caption entry despite successful physical mouse insertion."""

import itertools
import unittest
from unittest.mock import Mock, patch

from tools.splash_parity.move_entry_windows import observe_move_entry


class ContinuousEntryTests(unittest.TestCase):
    """Use deterministic time and fake OS state while retaining the gesture decision loop."""

    def setUp(self) -> None:
        self.observer = Mock()
        self.observer.origin.return_value = (100, 100)
        self.observer.in_move_loop.side_effect = [False, False, True]
        self.pointer = Mock()
        self.pointer.cursor.side_effect = lambda: (self.pointer.move_screen.call_args.args[0]
                                                  if self.pointer.move_screen.call_count else (350, 116))
        self.state = {}

    def run_entry(self):
        with patch("tools.splash_parity.move_entry_windows.time.perf_counter", side_effect=itertools.count(.01, .01)), \
                patch("tools.splash_parity.move_entry_windows.time.sleep"):
            return observe_move_entry(self.observer, 1, 42, self.pointer, (350, 116), 0, self.state)

    def test_horizontal_motion_continues_until_actual_entry_then_stops(self) -> None:
        self.run_entry()
        self.assertEqual([call.args[0] for call in self.pointer.move_screen.call_args_list], [(354, 116), (358, 116)])
        self.assertEqual([row["gui_in_move_size_owned"] for row in self.state["observations"]], [False, False, True])
        self.assertTrue(all(step["input_dispatched"] for step in self.state["steps"]))

    def test_already_entered_loop_emits_no_unnecessary_motion(self) -> None:
        self.observer.in_move_loop.side_effect = [True]
        self.run_entry()
        self.pointer.move_screen.assert_not_called()
        self.assertEqual(self.state["steps"], [])

    def test_never_entered_loop_caps_motion_at64_physical_pixels_and_rejects(self) -> None:
        self.observer.in_move_loop.side_effect = None
        self.observer.in_move_loop.return_value = False
        with self.assertRaisesRegex(RuntimeError, "within one second"):
            self.run_entry()
        self.assertEqual(self.pointer.move_screen.call_count, 16)
        self.assertEqual(self.pointer.move_screen.call_args.args[0], (414, 116))
        self.assertNotIn("entry_observed_ms", self.state)

    def test_foreign_target_keeps_failed_step_and_cannot_qualify(self) -> None:
        self.pointer.move_screen.side_effect = RuntimeError("foreign target")
        with self.assertRaisesRegex(RuntimeError, "foreign"):
            self.run_entry()
        self.assertFalse(self.state["steps"][0]["input_dispatched"])
        self.assertNotIn("entry_observed_ms", self.state)

    def test_actual_cursor_mismatch_rejects_motion(self) -> None:
        self.pointer.cursor.side_effect = None
        self.pointer.cursor.return_value = (350, 116)
        with self.assertRaisesRegex(RuntimeError, "did not reach"):
            self.run_entry()
        self.assertTrue(self.state["steps"][0]["input_dispatched"])

    def test_late_actual_entry_rejects_without_motion_or_acceptance(self) -> None:
        self.observer.in_move_loop.side_effect = [True]
        with patch("tools.splash_parity.move_entry_windows.time.perf_counter", return_value=1.001):
            with self.assertRaisesRegex(RuntimeError, "within one second"):
                observe_move_entry(self.observer, 1, 42, self.pointer, (350, 116), 0, self.state)
        self.pointer.move_screen.assert_not_called()
        self.assertTrue(self.state["observations"][0]["gui_in_move_size_owned"])

    def test_deadline_reached_between_query_and_dispatch_sends_no_late_input(self) -> None:
        with patch("tools.splash_parity.move_entry_windows.time.perf_counter", side_effect=[.999, 1.001]):
            with self.assertRaisesRegex(RuntimeError, "within one second"):
                observe_move_entry(self.observer, 1, 42, self.pointer, (350, 116), 0, self.state)
        self.pointer.move_screen.assert_not_called()
        self.assertFalse(self.state["steps"][0]["input_dispatched"])

    def test_owned_process_end_prevents_further_mouse_input(self) -> None:
        process = Mock()
        process.poll.return_value = 0
        with self.assertRaisesRegex(RuntimeError, "ended before"):
            observe_move_entry(self.observer, 1, 42, self.pointer, (350, 116), 0, self.state, process)
        self.observer.in_move_loop.assert_not_called()
        self.pointer.move_screen.assert_not_called()


if __name__ == "__main__":
    unittest.main()
