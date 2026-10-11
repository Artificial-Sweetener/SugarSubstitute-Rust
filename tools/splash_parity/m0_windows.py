# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Qualify M0 native timing and actual input through an owned Windows fixture.

Performance runs retain metadata and tiny display regions. Whole-source audits
belong to separate fixtures. Display observations include all sampling overhead.
"""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import subprocess
import time
from contextlib import nullcontext
from ctypes import wintypes
from dataclasses import asdict
from pathlib import Path

try:
    from .native_timing import analyse, distribution, integer, read_terminal, validate
    from .native_windows import MonitorInfo, NativePointer, NativeWindows, observe_exposure, write_json
    from .operational_windows import ObservationWindows, OperationalOutput, OwnedContention, affinity, error_causes
    from .region_capture import Region, RegionCapture
    from .move_entry_windows import observe_move_entry
    from .startup_windows import ReferenceProcesses
except ImportError:
    from native_timing import analyse, distribution, integer, read_terminal, validate
    from native_windows import MonitorInfo, NativePointer, NativeWindows, observe_exposure, write_json
    from operational_windows import ObservationWindows, OperationalOutput, OwnedContention, affinity, error_causes
    from region_capture import Region, RegionCapture
    from move_entry_windows import observe_move_entry
    from startup_windows import ReferenceProcesses


class Qpc:
    """Borrow the same Windows raw counter used by native timings and ROI observations."""
    def __init__(self):
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        for name in ("QueryPerformanceCounter", "QueryPerformanceFrequency"):
            call = getattr(self.kernel, name)
            call.argtypes, call.restype = [ctypes.POINTER(ctypes.c_int64)], ctypes.c_int
        self.frequency = self.read("QueryPerformanceFrequency")
        if self.frequency <= 0:
            raise RuntimeError("QPC frequency is not positive")

    def read(self, name="QueryPerformanceCounter"):
        value = ctypes.c_int64()
        if not getattr(self.kernel, name)(ctypes.byref(value)) or value.value <= 0:
            raise OSError(ctypes.get_last_error(), f"{name} failed")
        return value.value


def sample_record(sample):
    record = asdict(sample)
    record["bgra"] = sample.bgra.hex()
    return record


def pixels_close(left, right, tolerance=2):
    """Compare every observed color byte; screen alpha is unspecified."""
    return len(left) == len(right) and all(abs(a - b) <= tolerance
                                          for index, (a, b) in enumerate(zip(left, right)) if index % 4 != 3)


def pixels_changed(left, right):
    """Require each of the four ROI pixels to show a meaningful color transition."""
    return len(left) == len(right) == 16 and all(max(abs(a - b) for a, b in zip(left[i:i + 3], right[i:i + 3])) > 5
                                               for i in range(0, 16, 4))


def input_samples(pointer, capture, clock, samples, observations):
    """Return causal click-to-observed-pixel upper bounds, preserving complete QPC sampling brackets."""
    if samples != 32:
        raise ValueError("M0 input qualification requires exactly32 physical clicks")
    states, previous = [], None
    for index in range(samples):
        first = capture.sample()
        if first.qpc_frequency != clock.frequency:
            raise RuntimeError("ROI and input clocks differ")
        baseline_deadline = first.end_ticks + clock.frequency
        for _ in range(256):
            baseline = capture.sample()
            if baseline.end_ticks >= baseline_deadline:
                raise RuntimeError("Owned input baseline did not settle within one second")
            if pixels_close(first.bgra, baseline.bgra):
                break
            first = baseline
        else:
            raise RuntimeError("Input baseline exceeded bounded sampling inventory")
        if previous is not None and not pixels_close(previous, baseline.bgra):
            raise RuntimeError("Owned disclosure baseline changed without a measured click")
        start = clock.read()
        record = {"index": index, "input_begin_ticks": start, "baseline": sample_record(baseline),
                  "observations": [], "accepted": False}
        observations.append(record)
        pointer.press()
        pointer.release()
        record["input_dispatch_end_ticks"] = clock.read()
        for _ in range(256):
            observed = capture.sample()
            if observed.qpc_frequency != clock.frequency or observed.begin_ticks < start:
                raise RuntimeError("Input observation clock or ordering changed")
            record["observations"].append(sample_record(observed))
            if observed.end_ticks >= start + clock.frequency:
                raise RuntimeError("Physical caption input lacked visible response within one second")
            if pixels_changed(baseline.bgra, observed.bgra):
                if index >= 2 and not pixels_close(states[index % 2], observed.bgra):
                    raise RuntimeError("Visible disclosure did not return to its previously observed alternate state")
                if index < 2:
                    states.append(observed.bgra)
                else:
                    states[index % 2] = observed.bgra
                previous = observed.bgra
                record.update(accepted=True, input_to_visible_ms=(observed.end_ticks - start) * 1000 / clock.frequency,
                              observed_end_ticks=observed.end_ticks,
                              observer_sample_ms=(observed.end_ticks - observed.begin_ticks) * 1000 / clock.frequency)
                break
        else:
            raise RuntimeError("Input response exceeded bounded sampling inventory")
    result = distribution([item["input_to_visible_ms"] for item in observations])
    return {"input_to_visible": result, "p95_below_50ms": result["p95_ms"] < 50,
            "scope": "Physical press dispatch through actual owned composited ROI end observation, including full ownership/copy/sampling work; no overhead subtraction", "samples": observations}


def drag_path(observer, pointer, hwnd, pid, origin, cursor, direction, deadline, samples, clock, observations):
    """Measure32 actual movements from a qualified native anchor, checking owned OS loop on both sides."""
    if samples != 32:
        raise ValueError("M0 drag qualification requires exactly32 physical movements")
    schedule = time.perf_counter()
    for index in range(1, samples + 1):
        target_time = schedule + index * 2 / samples
        while (remaining := target_time - time.perf_counter()) > 0:
            time.sleep(min(.005, remaining))
        target = (cursor[0] + direction[0] * index * 4, cursor[1] + direction[1] * index * 2)
        expected = (origin[0] + direction[0] * index * 4, origin[1] + direction[1] * index * 2)
        record = dict(index=index, target=target, expected_origin=expected, accepted=False)
        observations.append(record)
        if not observer.in_move_loop(hwnd, pid):
            raise RuntimeError("Owned move loop ended before measured physical input")
        begin = clock.read()
        pointer.move_screen(target)
        record["dispatch_end_ticks"] = clock.read()
        record["wm_null_response_ms"] = observer.response(hwnd, pid)
        phase_deadline = min(deadline, time.perf_counter() + .75)
        while True:
            current = observer.origin(hwnd, pid)
            if time.perf_counter() >= phase_deadline:
                raise RuntimeError("Physical movement missed its strict position/deadline boundary")
            if max(abs(a - b) for a, b in zip(current, expected)) <= 2:
                break
            time.sleep(.005)
        actual_cursor = pointer.cursor()
        if max(abs(a - b) for a, b in zip(actual_cursor, target)) > 1:
            raise RuntimeError("Measured physical cursor departed its expected owned point")
        if not observer.in_move_loop(hwnd, pid):
            raise RuntimeError("Owned move loop ended during measured physical input")
        end = clock.read()
        record.update(begin_ticks=begin, end_ticks=end, actual_origin=current, actual_cursor=actual_cursor,
                      move_observed_ms=(end - begin) * 1000 / clock.frequency, accepted=True,
                      owned_move_loop_before=True, owned_move_loop_after=True)
    return {"samples": observations, "movement_observation": distribution([item["move_observed_ms"] for item in observations]),
            "scope": "Actual SendInput movement to observed owned window geometry; no compositor or source immutability claim"}


def physical_drag(observer, hwnd, pid, clock, state):
    native, samples = observer.native, 32
    client, origin = native.client(hwnd, pid), observer.origin(hwnd, pid)
    monitor = MonitorInfo()
    monitor.size = ctypes.sizeof(monitor)
    if not native.user.GetMonitorInfoW(native.user.MonitorFromWindow(hwnd, 2), ctypes.byref(monitor)):
        raise ctypes.WinError(ctypes.get_last_error())
    distance_x, distance_y = 32 * 4 + 16, 32 * 2 + 16
    dx = 1 if origin[0] + client.width + 64 + distance_x < monitor.work.right else -1
    dy = 1 if origin[1] + client.height + distance_y < monitor.work.bottom else -1
    if (origin[0] + client.width + 64 > monitor.work.right
            or dx < 0 and origin[0] - distance_x < monitor.work.left
            or dy < 0 and origin[1] - distance_y < monitor.work.top):
        raise RuntimeError("Owned work area lacks safe entry/calibration/32movement space")
    with NativePointer(native, hwnd, pid) as pointer:
        pointer.move((250, 16))
        start_point, started = pointer.cursor(), time.perf_counter()
        pointer.press()
        state["entry"] = {}
        observe_move_entry(observer, hwnd, pid, pointer, start_point, started, state["entry"])
        state["calibration"] = {}
        anchor, cursor = observer.calibrate_drag(hwnd, pid, pointer, observer.origin(hwnd, pid), pointer.cursor(),
                                                  (dx, dy), started + 10, state["calibration"])
        state["samples"] = []
        result = drag_path(observer, pointer, hwnd, pid, anchor, cursor, (dx, dy), started + 10, samples, clock, state["samples"])
        pointer.release()
        exit_deadline = time.perf_counter() + 1
        while observer.in_move_loop(hwnd, pid):
            if time.perf_counter() >= exit_deadline:
                raise RuntimeError("Owned actual move loop failed to leave after release")
            time.sleep(.005)
    return {**result, "entry": state["entry"], "calibration": state["calibration"]}


def campaign_decision(raw, drag, input_result):
    """Require complete evidence of the configured32-toggle/32-drag campaign.

    These coverage requirements assess this campaign, leaving arbitrary native
    distributions descriptive. Delegated movement must overlap actual measured
    owned OS-loop geometry observations in the same QPC clock.
    """
    validate(raw)
    movements, inputs = drag.get("samples"), input_result.get("samples")
    if (not isinstance(movements, list) or len(movements) != 32
            or not isinstance(inputs, list) or len(inputs) != 32
            or any(sample.get("accepted") is not True for sample in movements + inputs)):
        raise ValueError("M0 campaign requires32 completed physical movements and32 visible inputs")
    previous_end = raw["begin_ticks"]
    for sample in movements:
        begin = integer(sample.get("begin_ticks"), "physical movement begin", previous_end, raw["end_ticks"])
        end = integer(sample.get("end_ticks"), "physical movement end", begin, raw["end_ticks"])
        if sample.get("owned_move_loop_before") is not True or sample.get("owned_move_loop_after") is not True:
            raise ValueError("Physical campaign movement lacks actual owned OS-loop observations")
        previous_end = end
    first, last = movements[0]["begin_ticks"], movements[-1]["end_ticks"]
    summary = analyse(raw)
    required = ("windows_parent", "window_event", "completion_drain", "redraw", "loop_progress")
    kinds = {kind: summary["callbacks_by_kind"][kind]["authored"]["count"] for kind in required}
    delegated = [sample["sequence"] for sample in raw["callbacks"] if sample["kind"] == "windows_parent"
                 and sample["delegated_drag"] is not None
                 and sample["delegated_drag"]["begin_ticks"] < last
                 and sample["delegated_drag"]["end_ticks"] > first]
    count = summary["fresh_producer_submission_intervals"]["count"]
    return {"required_fresh_producer_intervals": 32, "fresh_producer_intervals_observed": count,
            "required_callback_kind_counts": kinds, "physical_drag_qpc_span": {"begin_ticks": first, "end_ticks": last},
            "overlapping_delegated_parent_sequences": delegated,
            "accepted": summary["complete_evidence"] and count >= 32 and all(kinds.values()) and bool(delegated),
            "scope": "Coverage of32 visible toggles and32 physical movements; native and visible latency budgets assessed separately"}


def performance_command(executable, receipt):
    return [str(executable), "--duration-ms", "45000", "--timing-audit", "--receipt", str(receipt)]


def run(args):
    executable = args.executable.resolve(strict=True)
    if os.name == "nt" and args.output.resolve().drive.lower() != "d:":
        raise ValueError("Native qualification artifacts and temporary paths must use D:")
    args.output.mkdir(parents=True, exist_ok=False)
    temporary = args.output / "temporary"
    temporary.mkdir()
    environment = os.environ.copy()
    environment.update(TEMP=str(temporary), TMP=str(temporary), PYTHONDONTWRITEBYTECODE="1")
    digest = hashlib.sha256(executable.read_bytes()).hexdigest()
    native, processes, clock = NativeWindows(), ReferenceProcesses(), Qpc()
    observer = ObservationWindows(native)
    terminal = args.output / "terminal.json"
    command = performance_command(executable, terminal)
    source_files = ("m0_windows.py", "native_timing.py", "native_windows.py", "region_capture.py",
                    "operational_windows.py", "move_entry_windows.py", "startup_windows.py", "contention_worker.py")
    harness = {name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest() for name in source_files}
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment, creationflags=0x08000000)
    output = OperationalOutput(process, args.output)
    primary, hwnd, cleanup = None, None, []
    state = {"schema_version": 1, "harness_sha256": harness, "command": command, "executable_sha256": digest, "pid": process.pid,
             "timing_source_audit_enabled": False, "qualification_accepted": False,
             "preparation_cadence": "Controlled by exact host configuration; this fixture does not synthesize producer cadence",
             "source_and_compositor_frame_age": "Not measured by performance fixture"}
    try:
        identity_pid, identity_path, creation = processes.identity(int(process._handle))
        if (identity_pid, identity_path) != (process.pid, executable) or not processes.alive(int(process._handle)):
            raise RuntimeError("Owned native process identity differs from the requested exact executable")
        state["creation_ticks"] = creation
        hwnd = native.find_window(process, time.perf_counter() + 5)
        state["hwnd"] = hwnd
        thread_pid = wintypes.DWORD()
        ui_thread = native.user.GetWindowThreadProcessId(hwnd, ctypes.byref(thread_pid))
        if not ui_thread or thread_pid.value != process.pid:
            raise RuntimeError("Actual window UI thread does not belong to the owned process")
        state["ui_thread_id"] = ui_thread
        observe_exposure(native, hwnd, process, time.perf_counter() + 5)
        mask = affinity(args.affinity_cpus) if args.affinity_cpus else None
        state["affinity"] = observer.set_affinity(hwnd, process.pid, mask) if mask is not None else None
        load = OwnedContention(observer, args.workers, mask) if args.workers else None
        try:
            with load if load is not None else nullcontext():
                state["drag_state"] = {}
                state["drag"] = physical_drag(observer, hwnd, process.pid, clock, state["drag_state"])
                client = native.client(hwnd, process.pid)
                region = Region(round(30 * client.scale), round(400 * client.scale), 2, 2)
                with RegionCapture(hwnd, process.pid, region, creation_ticks=creation) as capture:
                    with NativePointer(native, hwnd, process.pid) as pointer:
                        pointer.move((489, 16))
                        state["input_samples"] = []
                        state["input"] = input_samples(pointer, capture, clock, 32, state["input_samples"])
        finally:
            if load is not None:
                state["contention"] = load.receipt()
        native.post(hwnd, process.pid, 0x0010, 0, 0)
        output.collect("graceful_close")
        if process.returncode != 0:
            raise RuntimeError(f"Owned native timing fixture exit {process.returncode}")
        if hashlib.sha256(executable.read_bytes()).hexdigest() != digest:
            raise RuntimeError("Native executable changed during qualification")
        raw = read_terminal(terminal)
        if raw["ui_thread_id"] != ui_thread:
            raise RuntimeError("Terminal native timing UI thread differs from the actual owned window")
        if raw["frequency"] != clock.frequency:
            raise RuntimeError("Terminal and external observation QPC frequencies differ")
        state["native_timings"] = analyse(raw)
        state["campaign"] = campaign_decision(raw, state["drag"], state["input"])
        state["terminal_sha256"] = hashlib.sha256(terminal.read_bytes()).hexdigest()
        state["qualification_accepted"] = state["native_timings"]["accepted"] and state["campaign"]["accepted"] and state["input"]["p95_below_50ms"]
        return 0 if state["qualification_accepted"] else 1
    except BaseException as error:
        primary = error
        raise
    finally:
        if not output.complete:
            try:
                if hwnd is not None and process.poll() is None and native.owns(hwnd, process.pid):
                    native.post(hwnd, process.pid, 0x0010, 0, 0)
                    output.collect("failed_scenario_graceful_close")
            except BaseException as error:
                cleanup.append(error)
            if not output.complete:
                try:
                    output.failed_teardown()
                except BaseException as error:
                    cleanup.append(error)
        state["process_output"] = output.receipt()
        if primary is not None or cleanup:
            state["failure_causes"] = [cause for error in ([primary] if primary else []) + cleanup for cause in error_causes(error)]
            state["qualification_accepted"] = False
        try:
            write_json(args.output / "observation.json", state)
        except BaseException as error:
            cleanup.append(error)
        if cleanup:
            raise BaseExceptionGroup("M0 observation and owned teardown failed", ([primary] if primary else []) + cleanup)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--workers", type=int, default=0)
    parser.add_argument("--affinity-cpus")
    args = parser.parse_args()
    if not 0 <= args.workers <= 8 or args.workers and not args.affinity_cpus:
        parser.error("Owned zero..8 worker load requires explicit logical CPU affinity")
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
