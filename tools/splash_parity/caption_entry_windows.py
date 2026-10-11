# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Compare physical caption gesture-to-OS-move-loop entry for owned Rust and production Qt windows."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import time
from pathlib import Path

try:
    from .foreground_launcher import ForegroundLauncher
    from .native_windows import ExposureFailure, NativePointer, NativeWindows, observe_compositor, write_json
    from .operational_windows import ObservationWindows, error_causes
    from .move_entry_windows import observe_move_entry
    from .startup_windows import ReferenceOwner, ReferenceProcesses, launch, reference_base
except ImportError:
    from foreground_launcher import ForegroundLauncher
    from native_windows import ExposureFailure, NativePointer, NativeWindows, observe_compositor, write_json
    from operational_windows import ObservationWindows, error_causes
    from move_entry_windows import observe_move_entry
    from startup_windows import ReferenceOwner, ReferenceProcesses, launch, reference_base


def caption_entry(native: NativeWindows, observer: ObservationWindows, hwnd: int,
                  process, state: dict[str, object]) -> dict[str, object]:
    """Observe a physical press, bounded horizontal caption motion and release through the actual OS move loop."""
    client = native.client(hwnd, process.pid)
    state.update(hwnd=hwnd, pid=process.pid, dpi=client.dpi,
                 client_size=[client.width, client.height], logical_caption_point=[250, 16],
                 before_origin=observer.origin(hwnd, process.pid), entry_observations=[], exit_observations=[],
                 physical_movement_while_pressed=False, entry_bound_ms=1000, exit_bound_ms=1000)
    if observer.in_move_loop(hwnd, process.pid):
        raise RuntimeError("Caption entry fixture began with an active owned OS move loop")
    with NativePointer(native, hwnd, process.pid) as pointer:
        pointer.move((250, 16))
        state["press_cursor_screen"] = pointer.cursor()
        started = time.perf_counter()
        pointer.press()
        state["physical_press_dispatched"] = True
        gesture: dict[str, object] = {}
        state["entry_gesture"] = gesture
        try:
            observed = observe_move_entry(observer, hwnd, process.pid, pointer, state["press_cursor_screen"],
                                          started, gesture, process)
        finally:
            state["entry_observations"] = gesture["observations"]
            state["physical_movement_while_pressed"] = any(step["input_dispatched"] for step in gesture["steps"])
        state.update(entry_observed_ms=(observed - started) * 1000,
                     entry_origin=observer.origin(hwnd, process.pid), entry_cursor_screen=pointer.cursor())
        released = time.perf_counter()
        pointer.release()
        state["physical_release_dispatched"] = True
        while True:
            if process.poll() is not None:
                raise RuntimeError("Owned caption process ended before OS move-loop exit")
            active = observer.in_move_loop(hwnd, process.pid)
            observed = time.perf_counter()
            state["exit_observations"].append({"elapsed_ms": (observed - released) * 1000,
                                                "gui_in_move_size_owned": active})
            if observed >= released + 1:
                raise RuntimeError("Owned OS move loop did not exit within one second after physical release")
            if not active:
                break
            time.sleep(.005)
        state.update(exit_observed_ms=(observed - released) * 1000,
                     after_origin=observer.origin(hwnd, process.pid))
    return {"caption_entry_qualified": True, "entry_observed_ms": state["entry_observed_ms"],
            "exit_observed_ms": state["exit_observed_ms"],
            "input_source": "SendInput caption press, at most 16 horizontal physical movements (+4,0), and release",
            "observation": "GetGUIThreadInfo GUI_INMOVESIZE with hwndMoveSize equal to the verified owned window",
            "physical_drag_qualified": False}


def drain_counts(process: subprocess.Popen[bytes], receipt: dict[str, object], phase: str) -> None:
    """Observe terminal output byte counts without retaining production session authentication bytes."""
    stdout, stderr = process.communicate(timeout=5)
    if not isinstance(stdout, bytes) or not isinstance(stderr, bytes) or type(process.returncode) is not int:
        raise RuntimeError("Caption fixture process did not yield byte streams and a terminal exit code")
    receipt.setdefault("process_output", []).append({"phase": phase, "exit_code": process.returncode,
                                                     "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
                                                     "content_retained": False})


def observe(arguments: argparse.Namespace, output: Path, native: NativeWindows) -> dict[str, object]:
    """Prepare the common user-gesture launcher, then measure one immutable production launch identity."""
    output.mkdir(parents=True, exist_ok=False)
    command, cwd, environment = launch(arguments, output)
    receipt: dict[str, object] = {"mode": arguments.mode, "command": command, "cwd": str(cwd),
                                "creation_flags": 0x08000000, "qualification_accepted": False,
                                "entrypoint_sha256": hashlib.sha256(Path(command[0]).read_bytes()).hexdigest(),
                                "cache_policy": "Existing application and font caches retained",
                                "observer_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    receipt["shared_observer_sha256"] = {
        name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
        for name in ("native_windows.py", "operational_windows.py", "startup_windows.py", "foreground_launcher.py", "move_entry_windows.py")}
    base = None
    if arguments.mode == "reference":
        base = reference_base(Path(command[0]), environment)
        receipt["base_executable_sha256"] = hashlib.sha256(base.read_bytes()).hexdigest()
        receipt["reference_source_sha256"] = {
            name: hashlib.sha256((cwd / name).read_bytes()).hexdigest() for name in (
                "substitute/app/bootstrap/shared_splash_host.py", "substitute/presentation/shell/splash_window.py")}
    state: dict[str, object] = {}
    fixture = ForegroundLauncher(native)
    process = owner = None
    primary: BaseException | None = None
    cleanup: list[BaseException] = []
    entered_fixture = False
    drained = False
    try:
        fixture.__enter__()
        entered_fixture = True
        receipt["foreground_launch_setup"] = fixture.receipt
        started = time.perf_counter()
        process = subprocess.Popen(command, cwd=cwd, env=environment, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, creationflags=0x08000000)
        window_process = process
        deadline = started + arguments.duration_ms / 1000
        if arguments.mode == "reference":
            owner = ReferenceOwner(process, Path(command[0]), base, ReferenceProcesses())
            owner.bind(deadline)
            window_process = owner
            receipt["reference_process_owner"] = owner.metadata
        fixture.allow_product(window_process.pid)
        hwnd = native.find_window(window_process, deadline)
        _, painted, exposed = observe_compositor(native, hwnd, window_process, deadline)
        fixture.product_exposed()
        receipt.update(owned_exposed_ms=(exposed - started) * 1000, compositor_mascot_ms=(painted - started) * 1000)
        result = caption_entry(native, ObservationWindows(native), hwnd, window_process, state)
        receipt["caption_entry"] = result
        native.post(hwnd, window_process.pid, 0x0010, 0, 0)
        drain_counts(process, receipt, "graceful_close")
        drained = True
        if process.returncode != 0:
            raise RuntimeError(f"Caption fixture exited with status {process.returncode}")
    except BaseException as error:
        primary = error
        if isinstance(error, ExposureFailure):
            receipt["exposure_state"] = error.state
    finally:
        receipt["foreground_launch_setup"] = fixture.receipt
        if owner is not None:
            try:
                owner.close()
            except BaseException as error:
                cleanup.append(error)
        if process is not None and not drained:
            try:
                if process.poll() is None:
                    process.terminate()
                drain_counts(process, receipt, "owned_teardown")
            except BaseException as error:
                cleanup.append(error)
        if entered_fixture:
            try:
                fixture.__exit__(None, None, None)
            except BaseException as error:
                cleanup.append(error)
        try:
            write_json(output / "caption-state.json", state)
        except BaseException as error:
            cleanup.append(error)
    failures = ([primary] if primary is not None else []) + cleanup
    if failures:
        receipt["failure_causes"] = [cause for error in failures for cause in error_causes(error)]
        write_json(output / "failure.json", receipt)
        if len(failures) == 1:
            raise failures[0]
        raise BaseExceptionGroup("Caption entry and owned cleanup failures", failures)
    receipt["qualification_accepted"] = True
    write_json(output / "caption-entry.json", receipt)
    return receipt


def main() -> int:
    """Measure one sequential owned window; preserve first-launch cache state and original lifetime."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True, choices=("rust", "reference"))
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--interpreter", type=Path)
    parser.add_argument("--legacy-root", type=Path)
    parser.add_argument("--temporary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--locale", default="en", choices=("en", "zh-Hans", "ja", "ko", "es"))
    parser.add_argument("--light", action="store_true")
    parser.add_argument("--duration-ms", type=int, default=10000)
    args = parser.parse_args()
    if not 5000 <= args.duration_ms <= 30000:
        parser.error("Caption entry requires an owned 5000 through 30000 ms maximum lifetime")
    if args.mode == "rust" and args.executable is None:
        parser.error("Rust mode requires --executable")
    if args.mode == "reference" and (args.interpreter is None or args.legacy_root is None):
        parser.error("Reference mode requires --interpreter and --legacy-root")
    observe(args, args.output, NativeWindows())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
