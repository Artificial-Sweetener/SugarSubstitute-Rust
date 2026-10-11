# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Reject foreign input targets, missing activation and incomplete fixture cleanup."""

import ctypes
import itertools
import unittest
from unittest.mock import MagicMock, Mock, patch

from tools.splash_parity.foreground_launcher import ActivationPointer, ForegroundLauncher, Message, launcher_bounds


class ForegroundFixtureTests(unittest.TestCase):
    """Fake Windows boundaries while preserving real-gesture and owned-lifecycle decisions."""

    def setUp(self) -> None:
        """Provide owned native handles and deterministic explicit exposure facts."""
        self.native = Mock()
        self.native.client.return_value = Mock(width=160, height=48, scale=1)
        self.native.cursor_work_area.return_value = [0, 0, 1920, 1080]
        self.native.user.CreateWindowExW.return_value = 101
        self.native.user.PeekMessageW.return_value = False
        self.native.user.DestroyWindow.return_value = True
        self.fixture = ForegroundLauncher(self.native)
        self.fixture.hwnd = 101
        self.pointer = MagicMock()
        self.pointer.__enter__.return_value = self.pointer

    def state(self, foreground: bool) -> dict[str, object]:
        """Keep foreground admission independent from mocked input insertion success."""
        return {"foreground_owned": foreground, "occluding_windows": []}

    def test_native_message_abi_and_safe_position(self) -> None:
        """The message layout and fixture/client separation are explicit."""
        self.assertEqual(ctypes.sizeof(Message), 48 if ctypes.sizeof(ctypes.c_void_p) == 8 else 32)
        self.assertEqual(launcher_bounds([0, 0, 1920, 1080], 1), (8, 8, 160, 48))
        with self.assertRaisesRegex(RuntimeError, "No owned launcher position"):
            launcher_bounds([0, 0, 558, 558], 1)

    def test_actual_click_must_be_followed_by_observed_foreground(self) -> None:
        """A click inserts input; exposure facts independently establish launch admission."""
        self.native.exposure_state.side_effect = [self.state(False), self.state(False), self.state(True), self.state(True)]
        with patch("tools.splash_parity.foreground_launcher.ActivationPointer", return_value=self.pointer), \
                patch("tools.splash_parity.foreground_launcher.time.perf_counter", side_effect=itertools.count(0, .1)), \
                patch("tools.splash_parity.foreground_launcher.time.sleep"):
            self.fixture.activate()
        self.pointer.press.assert_called_once()
        self.pointer.release.assert_called_once()
        self.assertTrue(self.fixture.receipt["before_product_launch"]["foreground_owned"])

    def test_delivered_click_without_foreground_is_rejected(self) -> None:
        """Input insertion alone cannot qualify the setup or start a product timer."""
        self.native.exposure_state.return_value = self.state(False)
        with patch("tools.splash_parity.foreground_launcher.ActivationPointer", return_value=self.pointer), \
                patch("tools.splash_parity.foreground_launcher.time.perf_counter", side_effect=itertools.count(0, .5)), \
                patch("tools.splash_parity.foreground_launcher.time.sleep"):
            with self.assertRaisesRegex(RuntimeError, "did not establish owned foreground"):
                self.fixture.activate()
        self.pointer.__exit__.assert_called_once()
        self.assertIn("activation_deadline_state", self.fixture.receipt)

    def test_human_foreground_change_after_release_rejects_launch(self) -> None:
        """Prelaunch user input is preserved instead of reclaiming activation."""
        self.native.exposure_state.side_effect = [self.state(False), self.state(True), self.state(False)]
        with patch("tools.splash_parity.foreground_launcher.ActivationPointer", return_value=self.pointer):
            with self.assertRaisesRegex(RuntimeError, "changed before product launch"):
                self.fixture.activate()

    def test_foreign_or_occluded_click_target_is_rejected(self) -> None:
        """Click-to-activate is restricted to the owned window, including the physical hit."""
        pointer = object.__new__(ActivationPointer)
        pointer.native, pointer.hwnd, pointer.pid = self.native, 101, 42
        self.native.user.WindowFromPoint.return_value = 7
        self.native.user.GetAncestor.return_value = 999
        with self.assertRaisesRegex(RuntimeError, "unoccluded owned client"):
            pointer.owned_hit((10, 10))
        self.assertEqual(pointer.last_probe["hit_root_hwnd"], 999)
        self.assertIn("owned_screen_bounds", pointer.last_probe)
        self.native.user.GetAncestor.return_value = 101
        self.native.occluded.return_value = True
        with self.assertRaisesRegex(RuntimeError, "unoccluded owned client"):
            pointer.owned_hit((10, 10))

    def test_failed_activation_destroys_only_owned_fixture(self) -> None:
        """Setup failures retire the window on its creating thread."""
        self.fixture.hwnd = None
        with patch.object(self.fixture, "activate", side_effect=RuntimeError("activation rejected")):
            with self.assertRaisesRegex(RuntimeError, "activation rejected"):
                self.fixture.__enter__()
        self.native.user.DestroyWindow.assert_called_once_with(101)
        self.assertIsNone(self.fixture.hwnd)
        self.assertEqual(self.native.user.CreateWindowExW.call_args.args[1], "BUTTON")

    def test_lost_hwnd_identity_cannot_destroy_foreign_window(self) -> None:
        """A reused or externally destroyed fixture handle is an observed cleanup failure."""
        self.native.owns.return_value = False
        with self.assertRaisesRegex(RuntimeError, "lost HWND ownership"):
            self.fixture.__exit__(None, None, None)
        self.native.user.DestroyWindow.assert_not_called()

    def test_cleanup_failure_keeps_original_observation_cause(self) -> None:
        """Native teardown errors preserve both boundary causes."""
        self.native.user.DestroyWindow.return_value = False
        with patch("tools.splash_parity.foreground_launcher.ctypes.WinError", return_value=OSError("destroy failed"), create=True), \
                patch("tools.splash_parity.foreground_launcher.ctypes.get_last_error", return_value=1, create=True):
            original = RuntimeError("observation rejected")
            with self.assertRaises(BaseExceptionGroup) as failure:
                self.fixture.__exit__(RuntimeError, original, None)
        self.assertEqual(failure.exception.exceptions[0], original)
        self.assertEqual(str(failure.exception.exceptions[1]), "destroy failed")

    def test_message_inventory_and_hide_failures_are_observable(self) -> None:
        """A flooded fixture queue or a still-visible fixture cannot be silently retired."""
        self.native.user.PeekMessageW.return_value = True
        with self.assertRaisesRegex(RuntimeError, "bounded message inventory"):
            self.fixture.pump()
        self.native.user.IsWindowVisible.return_value = True
        with self.assertRaisesRegex(RuntimeError, "did not hide"):
            self.fixture.product_exposed()

    def test_permission_targets_only_verified_product_while_fixture_foreground(self) -> None:
        """Foreground permission is a specific PID grant, independently checked after launch ownership."""
        self.native.exposure_state.return_value = self.state(True)
        self.fixture.allow_product(42)
        self.native.user.AllowSetForegroundWindow.assert_called_once_with(42)
        self.native.exposure_state.return_value = self.state(False)
        with self.assertRaisesRegex(RuntimeError, "lost foreground"):
            self.fixture.allow_product(43)
        self.native.user.AllowSetForegroundWindow.assert_called_once()
        self.native.exposure_state.return_value = self.state(True)
        with self.assertRaises(ValueError):
            self.fixture.allow_product(0)

    def test_already_activated_verified_product_needs_no_permission_grant(self) -> None:
        """A fast product may legitimately own foreground before observer-side permission bookkeeping."""
        self.native.exposure_state.return_value = {**self.state(False), "foreground_pid": 42}
        self.fixture.allow_product(42)
        self.native.user.AllowSetForegroundWindow.assert_not_called()
        self.assertIsNone(self.fixture.receipt["activation_permission"]["api"])


if __name__ == "__main__":
    unittest.main()
