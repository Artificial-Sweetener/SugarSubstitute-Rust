# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Measure installed native window movement and message latency on an inactive desktop.

This sends messages only to verified test-owned windows. It never switches desktops,
injects physical input, or claims to measure a human pointer gesture.
"""
from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import time

from tools.splash_parity.offscreen_native import PrivateDesktop
from tools.splash_parity.offscreen_lifecycle import resolve_shortcut


def summarize(samples):
    """Return millisecond quantiles with a separate count so missing evidence cannot pass."""
    values = sorted(samples)
    if not values:
        raise RuntimeError("No responsiveness samples were collected")
    return dict(count=len(values), median_ms=values[len(values) // 2],
                p95_ms=values[min(len(values) - 1, int(len(values) * .95))], max_ms=values[-1])


def trial(options):
    """Exercise real startup, disclosure, repeated positioning, main reveal and normal closure."""
    desktop = PrivateDesktop()
    user = ctypes.WinDLL("user32", use_last_error=True)
    send = user.SendMessageTimeoutW
    send.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM,
                     wintypes.UINT, wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)]
    send.restype = wintypes.LPARAM
    executable = options.installation.resolve(strict=True) / "sugarsubstitute.exe"
    directory = Path("C:/Windows")
    if options.shortcut:
        executable, directory = resolve_shortcut(options.shortcut, executable)
    evidence = dict(executable=str(executable), private_desktop=desktop.name,
                    physical_input=False, disclosure=options.disclosure,
                    launch_method="resolved shortcut" if options.shortcut else "direct executable",
                    splash_hwnds=[], main_hwnds=[], natural_cleanup=False)
    samples = {kind + "_" + action: [] for kind in ("splash", "main")
               for action in ("ping", "enter_move", "move", "exit_move")}

    def probe(hwnd, kind, action, message):
        """Time only a synchronous native message, without entering a physical input loop."""
        result = ctypes.c_size_t()
        before = time.perf_counter()
        if not send(hwnd, message, 0, 0, 3, 1000, ctypes.byref(result)):
            raise RuntimeError(f"{kind} failed a one-second native {action} probe")
        samples[kind + "_" + action].append((time.perf_counter() - before) * 1000)
    disclosed = False
    main_at = None
    started = time.perf_counter()
    close_at = None
    try:
        evidence["root_pid"] = desktop.launch(executable, directory)
        while time.perf_counter() - started < options.timeout:
            desktop.discover()
            windows = [value for value in desktop.windows() if value["visible"]]
            now = time.perf_counter()
            for window in windows:
                hwnd = window["hwnd"]
                if window["title"] != "SugarSubstitute":
                    raise RuntimeError("Unexpected test window: " + window["title"])
                kind = "splash" if window["splash"] else "main"
                if hwnd not in evidence[kind + "_hwnds"]:
                    evidence[kind + "_hwnds"].append(hwnd)
                    evidence.setdefault("first_visible_seconds", now - started)
                if kind == "main" and main_at is None:
                    main_at = now
                    evidence["main_seconds"] = now - started
                    evidence["main_caption_hits"] = desktop.caption_hits(hwnd)
                if kind == "splash" and options.disclosure and not disclosed:
                    desktop.click_control(hwnd, 1)
                    disclosed = True
                evidence["console_observed"] = evidence.get("console_observed", False) or window["console"]
                if close_at is not None:
                    continue
                desktop.assert_window(hwnd)
                probe(hwnd, kind, "ping", 0)
                probe(hwnd, kind, "enter_move", 0x231)
                left, top, _, _ = window["rect"]
                try:
                    for movement in range(8):
                        offset = 3 if movement % 2 == 0 else 0
                        before = time.perf_counter()
                        desktop.move(hwnd, left + offset, top)
                        samples[kind + "_move"].append((time.perf_counter() - before) * 1000)
                finally:
                    probe(hwnd, kind, "exit_move", 0x232)
                if window["console"]:
                    evidence["console_tail"] = desktop.console_tail(hwnd)
                if kind == "splash":
                    evidence["retry_observed"] = evidence.get("retry_observed", False) or window["retry"]
            if main_at is not None and close_at is None and now - main_at >= options.main_hold:
                for window in windows:
                    desktop.close_window(window["hwnd"])
                close_at = now
            if not desktop.alive():
                evidence["natural_cleanup"] = close_at is not None
                break
            if close_at is not None and now - close_at > 35:
                raise RuntimeError("Test-owned processes did not close naturally")
            time.sleep(.02)
        evidence["remaining_pids"] = desktop.alive()
        if not evidence["natural_cleanup"] or evidence["remaining_pids"]:
            raise RuntimeError("Startup or natural closure did not complete")
        evidence["timings"] = {name: summarize(values) for name, values in samples.items()}
        if len(evidence["splash_hwnds"]) != 1 or len(evidence["main_hwnds"]) != 1:
            raise RuntimeError("Unexpected native window replacement")
        if options.disclosure and not evidence.get("console_observed"):
            raise RuntimeError("Console disclosure was not observed")
        if not any(hit["hit"] == 2 for hit in evidence.get("main_caption_hits", [])):
            raise RuntimeError("The main window exposed no native caption hit in its drag region")
        for name, timing in evidence["timings"].items():
            if timing["p95_ms"] > options.p95_limit or timing["max_ms"] > options.maximum_limit:
                raise RuntimeError(f"{name} exceeded the qualification latency limit: {timing}")
    except BaseException as cause:
        evidence["error"] = str(cause)
        evidence["remaining_pids"] = desktop.alive()
        raise
    finally:
        evidence["elapsed_seconds"] = time.perf_counter() - started
        evidence["exit_codes"] = desktop.exit_codes()
        evidence["samples_ms"] = samples
        options.output.write_text(json.dumps(evidence, indent=2), encoding="utf-8")
        desktop.close()
    print(json.dumps({key: value for key, value in evidence.items()
                     if key not in ("samples_ms", "console_tail")}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installation", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--shortcut", type=Path)
    parser.add_argument("--disclosure", action="store_true")
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--main-hold", type=float, default=10)
    parser.add_argument("--p95-limit", type=float, default=50)
    parser.add_argument("--maximum-limit", type=float, default=250)
    options = parser.parse_args()
    if options.output.resolve().drive.casefold() != "d:":
        parser.error("Native qualification reports belong on D:")
    trial(options)


if __name__ == "__main__":
    main()
