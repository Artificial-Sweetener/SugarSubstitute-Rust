# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Observe a genuine bounded horizontal caption gesture through actual owned OS move-loop entry."""

from __future__ import annotations

import time


def observe_move_entry(observer, hwnd: int, pid: int, pointer, start_point: tuple[int, int],
                       press_started: float, state: dict[str, object], process=None) -> float:
    """Continue physical caption motion while pending; reject late entry without extending the press deadline."""
    state.update(start_cursor_screen=start_point, maximum_physical_steps=16, physical_step_pixels=[4, 0],
                 entry_bound_ms=1000, timer_origin="before physical press dispatch", observations=[], steps=[])
    deadline = press_started + 1
    while True:
        if process is not None and process.poll() is not None:
            raise RuntimeError("Owned caption process ended before OS move-loop entry")
        active = observer.in_move_loop(hwnd, pid)
        observed = time.perf_counter()
        state["observations"].append({"after_press_ms": (observed - press_started) * 1000,
                                       "gui_in_move_size_owned": active})
        if observed >= deadline:
            state.update(rejected_window_origin=observer.origin(hwnd, pid), rejected_cursor_screen=pointer.cursor())
            raise RuntimeError("Physical caption gesture did not enter the owned OS move loop within one second")
        if active:
            state["entry_observed_ms"] = (observed - press_started) * 1000
            return observed
        if len(state["observations"]) >= 512:
            raise RuntimeError("Caption entry observation exceeded its bounded state inventory")
        if len(state["steps"]) < 16:
            index = len(state["steps"]) + 1
            target = (start_point[0] + index * 4, start_point[1])
            step = {"index": index, "cursor_screen_target": target, "input_dispatched": False,
                    "dispatch_after_press_ms": (time.perf_counter() - press_started) * 1000}
            state["steps"].append(step)
            if step["dispatch_after_press_ms"] >= 1000:
                state.update(rejected_window_origin=observer.origin(hwnd, pid), rejected_cursor_screen=pointer.cursor())
                raise RuntimeError("Physical caption gesture did not enter the owned OS move loop within one second")
            pointer.move_screen(target)
            step.update(input_dispatched=True, observed_cursor_screen=pointer.cursor(),
                        observed_window_origin=observer.origin(hwnd, pid),
                        observed_after_press_ms=(time.perf_counter() - press_started) * 1000)
            if max(abs(a - b) for a, b in zip(step["observed_cursor_screen"], target)) > 1:
                raise RuntimeError("Actual caption gesture cursor did not reach its owned physical target")
        time.sleep(.005)
