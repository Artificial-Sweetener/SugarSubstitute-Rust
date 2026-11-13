# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Qualify installed startup and cleanup without showing or controlling the input desktop.

Run with python -m tools.splash_parity.offscreen_lifecycle. Supply a separately
provisioned installation/configuration/profile and a report path on D:.
"""
from __future__ import annotations

import argparse
import json
import time
import sys
from pathlib import Path


def verify(evidence, expected_splashes, maximum_startup, require_main, require_disclosure):
    """Reject replacement windows, movement resets, missing UI and unfinished cleanup."""
    if evidence.get("error"):
        raise RuntimeError(evidence["error"])
    if evidence["remaining_pids"]:
        raise RuntimeError("Owned test processes remain after shutdown")
    if evidence["first_visible_seconds"] is None:
        raise RuntimeError("No splash became visible")
    if evidence["first_visible_seconds"] > maximum_startup:
        raise RuntimeError("The first visible splash exceeded the qualification startup limit")
    if len(evidence["splash_hwnds"]) != expected_splashes:
        raise RuntimeError("The launch created an unexpected number of splash windows")
    if expected_splashes == 1 and not evidence["position_preserved"]:
        raise RuntimeError("The moved splash changed its original handle or position")
    if require_main and not evidence["main_observed"]:
        raise RuntimeError("The actual main window did not become visible")
    if require_disclosure and not evidence["console_observed"]:
        raise RuntimeError("The same splash did not expose its native output control")
    if evidence.get("scenario") == "hidden-main-close":
        if "hidden_main_close_seconds" not in evidence:
            raise RuntimeError("The hidden main close was not exercised")
        if "retry_clicked_seconds" not in evidence or not evidence["main_observed"]:
            raise RuntimeError("Hidden main closure did not recover through splash Retry")


def resolve_shortcut(shortcut, expected):
    """Resolve an actual .lnk without invoking Explorer or changing its window placement."""
    import pythoncom
    from win32com.shell import shell
    pythoncom.CoInitialize()
    try:
        link = pythoncom.CoCreateInstance(shell.CLSID_ShellLink, None,
            pythoncom.CLSCTX_INPROC_SERVER, shell.IID_IShellLink)
        link.QueryInterface(pythoncom.IID_IPersistFile).Load(str(shortcut))
        target = Path(link.GetPath(shell.SLGP_RAWPATH)[0]).resolve(strict=True)
        if target != expected.resolve(strict=True) or link.GetArguments():
            raise RuntimeError("The shortcut must name the tested executable with no extra arguments")
        return target, Path(link.GetWorkingDirectory() or target.parent)
    finally:
        pythoncom.CoUninitialize()


def trial(options):
    from tools.splash_parity.offscreen_native import PrivateDesktop
    executable = options.installation.resolve(strict=True) / "sugarsubstitute.exe"
    directory = Path("C:/Windows")
    if options.shortcut:
        executable, directory = resolve_shortcut(options.shortcut, executable)
    desktop = PrivateDesktop()
    evidence = dict(first_visible_seconds=None, splash_hwnds=[], main_observed=False,
        console_observed=False, position_preserved=True, remaining_pids=[], events=[],
        scenario=options.scenario, launch_method="resolved .lnk target" if options.shortcut
        else "direct executable with no arguments", private_desktop=desktop.name)
    moved = None
    closure = None
    duplicate = False
    retry_clicked = False
    first = None
    previous = None
    started = time.perf_counter()
    try:
        if options.diagnostics:
            helper = Path(__file__).with_name('offscreen_child.py').resolve()
            root = desktop.launch(Path(sys.executable), directory,
                [str(helper), str(executable), str(directory), str(options.diagnostics)])
        else:
            root = desktop.launch(executable, directory)
        evidence["root_pid"] = root
        while time.perf_counter() - started < options.timeout:
            desktop.discover()
            now = time.perf_counter() - started
            windows = desktop.windows()
            visible = [window for window in windows if window["visible"]]
            if windows != previous:
                evidence["events"].append(dict(seconds=now, windows=windows))
                previous = windows
            for window in visible:
                if window["cls"] == "#32770":
                    raise RuntimeError("Startup dialog: " + str(desktop.dialog_text(window["hwnd"])))
                if window["title"] != "SugarSubstitute":
                    raise RuntimeError("Unexpected owned window: " + window["title"])
                if not window["splash"]:
                    evidence["main_observed"] = True
                    continue
                hwnd = window["hwnd"]
                if hwnd not in evidence["splash_hwnds"]:
                    evidence["splash_hwnds"].append(hwnd)
                if first is None:
                    first = now
                    evidence["first_visible_seconds"] = now
                    left, top, right, bottom = window["rect"]
                    moved = (hwnd, (left + 83, top + 61, right + 83, bottom + 61))
                    desktop.move(hwnd, moved[1][0], moved[1][1])
                    if options.disclosure:
                        desktop.click_control(hwnd, 1)
                elif hwnd != moved[0] or tuple(window["rect"]) != moved[1]:
                    evidence["position_preserved"] = False
                evidence["console_observed"] |= window["console"]
                if options.scenario in ("retry", "hidden-main-close") and window["retry"] and not retry_clicked:
                    desktop.click_control(hwnd, 3)
                    retry_clicked = True
                    evidence["retry_clicked_seconds"] = now
            if options.scenario == "hidden-main-close" and first is not None and "hidden_main_close_seconds" not in evidence:
                hidden_main = next((window for window in windows if not window["visible"]
                    and not window["splash"] and window["title"] == "SugarSubstitute"), None)
                if hidden_main:
                    desktop.close_window(hidden_main["hwnd"])
                    evidence["hidden_main_hwnd"] = hidden_main["hwnd"]
                    evidence["hidden_main_close_seconds"] = now
            if first is not None and closure is None:
                if options.scenario == "duplicate" and not duplicate and now - first >= 1:
                    evidence["second_root_pid"] = desktop.launch(executable, directory)
                    duplicate = True
                ready = now - first >= options.hold
                if options.scenario in ("main", "hidden-main-close"):
                    ready &= evidence["main_observed"]
                if options.scenario == "retry":
                    ready &= retry_clicked and now - evidence["retry_clicked_seconds"] >= options.hold
                if ready:
                    if options.scenario == "root-crash":
                        desktop.stop(root)
                    elif options.scenario == "watchdog":
                        desktop.suspend(root)
                    else:
                        for window in visible:
                            desktop.close_window(window["hwnd"])
                    closure = now
                    evidence["close_requested_seconds"] = now
            alive = desktop.alive()
            if not alive:
                break
            if closure is not None and now - closure > 35:
                raise RuntimeError("Owned family did not exit within the cleanup qualification limit")
            time.sleep(.01)
        desktop.discover()
        evidence["remaining_pids"] = desktop.alive()
        evidence["elapsed_seconds"] = time.perf_counter() - started
        verify(evidence, options.expect_splashes, options.maximum_startup,
               options.scenario in ("main", "hidden-main-close"), options.disclosure)
    except BaseException as cause:
        evidence["error"] = str(cause)
        evidence["remaining_pids"] = desktop.alive()
        raise
    finally:
        # Record natural cleanup before any emergency test-fixture termination.
        options.output.write_text(json.dumps(evidence, indent=2), encoding="utf-8")
        desktop.close()
    print(json.dumps({key: value for key, value in evidence.items() if key != "events"}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installation", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--shortcut", type=Path)
    parser.add_argument("--diagnostics", type=Path)
    parser.add_argument("--scenario", choices=("cancel", "main", "duplicate", "root-crash", "watchdog", "retry", "hidden-main-close"), default="cancel")
    parser.add_argument("--hold", type=float, default=8)
    parser.add_argument("--timeout", type=float, default=90)
    parser.add_argument("--maximum-startup", type=float, default=2)
    parser.add_argument("--expect-splashes", type=int, default=1)
    parser.add_argument("--disclosure", action="store_true")
    options = parser.parse_args()
    if options.output.resolve().drive.casefold() != "d:":
        parser.error("Native qualification reports belong on D:")
    if options.hold <= 0 or options.timeout <= options.hold or options.maximum_startup <= 0:
        parser.error("Qualification durations must be positive with a longer overall timeout")
    if options.diagnostics and options.scenario in ("root-crash", "watchdog", "duplicate"):
        parser.error("Root fault and repeated-entry trials require direct executable launch")
    trial(options)


if __name__ == "__main__":
    main()
