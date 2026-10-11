# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Reject false static CPU acceptance when process identity, visibility or lifecycle changes."""

import unittest
from unittest.mock import Mock, patch

from tools.splash_parity.operational_windows import ObservationWindows


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
