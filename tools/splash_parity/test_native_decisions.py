# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Reject false native acceptance when input, compositor or accessible evidence is missing."""

import unittest

from tools.splash_parity.native_decisions import accessible_controls, startup_decision


class NativeDecisionTests(unittest.TestCase):
    """Use external-observation-shaped fixtures, independently of native implementation."""

    def controls(self) -> list[dict[str, object]]:
        """Build two separately named and actionable observed native buttons."""
        return [dict(name=name, process_id=42, control_type="ControlType.Button", automation_id=str(index),
                     window_handle=index, has_keyboard_focus=False,
                     keyboard_focusable=True, enabled=True, invoke_supported=True,
                     offscreen=False, bounds=[0, 0, 46, 32])
                for index, name in enumerate(("Hide Comfy output", "Cancel loading"), start=1)]

    def virtual(self, identifier: str = "Close") -> dict[str, object]:
        """Mirror the standard Windows offscreen nonclient button proxy."""
        return dict(name=identifier, process_id=42, control_type="ControlType.Button",
                    automation_id=identifier, window_handle=0, has_keyboard_focus=False,
                    keyboard_focusable=False, enabled=True, invoke_supported=True,
                    offscreen=True, bounds=[0, 0, 47, 30])

    def test_accessible_buttons_are_owned_named_and_actionable(self) -> None:
        """Valid evidence passes and malformed/missing evidence fails."""
        names = ("Hide Comfy output", "Cancel loading")
        accessible_controls(self.controls(), 42, names)
        for field, invalid in (("name", ""), ("process_id", 43),
                               ("control_type", "ControlType.Pane"), ("keyboard_focusable", False),
                               ("invoke_supported", False), ("enabled", False),
                               ("offscreen", True), ("bounds", [0, 0, 0, 32]),
                               ("bounds", [0, 0, float("nan"), 32]),
                               ("automation_id", "3"), ("window_handle", 0)):
            with self.subTest(field=field):
                records = self.controls()
                records[0][field] = invalid
                with self.assertRaises(ValueError):
                    accessible_controls(records, 42, names)
        with self.assertRaises(ValueError):
            accessible_controls(self.controls()[:1], 42, names)

    def test_known_os_virtual_nonclient_controls_are_retained_and_scoped(self) -> None:
        """The two real peers remain required alongside observed standard OS proxies."""
        records = self.controls() + [self.virtual(identifier) for identifier in ("Minimize", "Maximize", "Close")]
        original = list(records)
        accessible_controls(records, 42, ("Hide Comfy output", "Cancel loading"))
        self.assertEqual(records, original)
        with self.assertRaises(ValueError):
            accessible_controls(records[1:], 42, ("Hide Comfy output", "Cancel loading"))

    def test_unexpected_virtual_or_visible_controls_fail(self) -> None:
        """No foreign, visible, focusable or unknown controls disappear through filtering."""
        names = ("Hide Comfy output", "Cancel loading")
        for field, invalid in (("process_id", 43), ("automation_id", "Unexpected"),
                               ("offscreen", False), ("keyboard_focusable", True),
                               ("has_keyboard_focus", True), ("enabled", False),
                               ("invoke_supported", False), ("window_handle", "0"),
                               ("name", ""), ("bounds", [0, 0, float("nan"), 30])):
            with self.subTest(field=field):
                virtual = self.virtual()
                virtual[field] = invalid
                with self.assertRaises(ValueError):
                    accessible_controls(self.controls() + [virtual], 42, names)
        with self.assertRaises(ValueError):
            accessible_controls(self.controls() + [self.virtual(), self.virtual()], 42, names)
        unexpected = self.controls()[0].copy()
        unexpected.update(automation_id="Unexpected", window_handle=3)
        with self.assertRaises(ValueError):
            accessible_controls(self.controls() + [unexpected], 42, names)

    def test_missing_or_late_external_observation_fails_startup(self) -> None:
        """Submission or window creation alone cannot establish displayed interactive startup."""
        self.assertTrue(startup_decision(500, 750)["accepted"])
        for visible, interactive in ((None, 2), (2, None), (500.01, 2),
                                      (2, 750.01), (-1, 2), (2, -1)):
            with self.subTest(visible=visible, interactive=interactive):
                self.assertFalse(startup_decision(visible, interactive)["accepted"])


if __name__ == "__main__":
    unittest.main()
