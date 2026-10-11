# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Validate external Windows accessibility and startup observations."""

from __future__ import annotations

import math
try:
    from .timing_policy import assess_timing
except ImportError:
    from timing_policy import assess_timing


def accessible_controls(records: list[dict[str, object]], pid: int, names: tuple[str, str]) -> None:
    """Validate owned caption peers while retaining Windows' known offscreen nonclient proxies."""
    caption = []
    virtual_ids = set()
    for record in records:
        if record["process_id"] != pid or record["control_type"] != "ControlType.Button":
            raise ValueError("Accessible caption control has foreign ownership or wrong role")
        bounds = record["bounds"]
        if (not isinstance(bounds, list) or len(bounds) != 4
                or any(type(value) not in (int, float) or not math.isfinite(value) for value in bounds)
                or bounds[2] <= 0 or bounds[3] <= 0):
            raise ValueError("Accessible control has invalid screen bounds")
        handle = record["window_handle"]
        if type(handle) is not int:
            raise ValueError("Accessible control has invalid native window metadata")
        if handle != 0:
            caption.append(record)
            continue
        identifier = record["automation_id"]
        if identifier not in ("Minimize", "Maximize", "Close") or identifier in virtual_ids:
            raise ValueError("Unrecognized or duplicated virtual nonclient accessibility control")
        if not isinstance(record["name"], str) or not record["name"].strip():
            raise ValueError("Virtual nonclient accessibility control has no OS-provided name")
        if record["offscreen"] is not True or record["keyboard_focusable"] is not False or record["has_keyboard_focus"] is not False:
            raise ValueError("Virtual nonclient accessibility control is visible or focusable")
        if record["enabled"] is not True or record["invoke_supported"] is not True:
            raise ValueError("Virtual nonclient accessibility control has unrecognized action metadata")
        virtual_ids.add(identifier)
    if len(caption) != 2:
        raise ValueError("The splash must expose exactly its two accessible caption buttons")
    if {record["automation_id"] for record in caption} != {"1", "2"} or len({record["window_handle"] for record in caption}) != 2:
        raise ValueError("Accessible caption peers have incorrect native identities")
    if {record["automation_id"]: record["name"] for record in caption} != {"1": names[0], "2": names[1]}:
        raise ValueError("Accessible names do not match the active locale and disclosure state")
    for record in caption:
        if not all(record[field] is True for field in ("keyboard_focusable", "enabled", "invoke_supported")):
            raise ValueError("Accessible caption control cannot receive keyboard input or Invoke")
        if record["offscreen"] is not False:
            raise ValueError("Accessible caption control is offscreen")


def startup_decision(visible_ms: float | None, interactive_ms: float | None, *, profile="strict") -> dict[str, object]:
    """Assess actual startup; busy derived reports add explicit budgets and retain strict flags."""
    visible = assess_timing(visible_ms, 500, profile=profile, inclusive=True)
    interactive = assess_timing(interactive_ms, 750, profile=profile, inclusive=True)
    result = {
        "visible_frame_ms": visible_ms,
        "interactive_control_ms": interactive_ms,
        "visible_within_500_ms": visible["strict_passed"],
        "interactive_within_750_ms": interactive["strict_passed"],
        "accepted": visible["accepted"] and interactive["accepted"],
    }
    if profile == "busy":
        result["timing_assessment"] = {"profile": profile, "visible": visible, "interactive": interactive,
                                       "strict_accepted": visible["strict_passed"] and interactive["strict_passed"],
                                       "accepted": result["accepted"]}
    return result
