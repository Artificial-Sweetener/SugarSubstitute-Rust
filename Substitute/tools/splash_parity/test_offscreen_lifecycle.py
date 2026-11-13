# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Ensure failed native evidence cannot be reported as a successful lifecycle run."""
import copy
import unittest
from tools.splash_parity.offscreen_lifecycle import verify


class LifecycleEvidenceTests(unittest.TestCase):
    def successful(self):
        return dict(first_visible_seconds=.4, splash_hwnds=[101], main_observed=True,
                    console_observed=True, position_preserved=True, remaining_pids=[])

    def test_single_persistent_window_and_completed_family_are_required(self):
        verify(self.successful(), 1, 2, True, True)
        for key, value in (("splash_hwnds", [101, 102]), ("position_preserved", False),
                           ("first_visible_seconds", None), ("first_visible_seconds", 2.1),
                           ("remaining_pids", [7]), ("main_observed", False),
                           ("console_observed", False), ("error", "claim validation failed")):
            with self.subTest(key=key, value=value):
                evidence = copy.deepcopy(self.successful())
                evidence[key] = value
                with self.assertRaises(RuntimeError):
                    verify(evidence, 1, 2, True, True)

    def test_baseline_reproduction_requires_two_actual_handles(self):
        with self.assertRaises(RuntimeError):
            verify(self.successful(), 2, 2, False, False)
        evidence = self.successful()
        evidence.update(splash_hwnds=[101, 102], position_preserved=False)
        verify(evidence, 2, 2, False, False)

    def test_hidden_main_failure_requires_injection_retry_and_actual_reveal(self):
        evidence = self.successful()
        evidence.update(scenario="hidden-main-close", hidden_main_close_seconds=10,
                        retry_clicked_seconds=12)
        verify(evidence, 1, 2, True, True)
        for key in ("hidden_main_close_seconds", "retry_clicked_seconds"):
            incomplete = copy.deepcopy(evidence)
            del incomplete[key]
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                verify(incomplete, 1, 2, True, True)


if __name__ == "__main__":
    unittest.main()
