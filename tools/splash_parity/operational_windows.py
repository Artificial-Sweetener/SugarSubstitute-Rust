# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Observe owned Windows splash CPU, native mouse drag, responsiveness and real monitor DPI."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import math
import os
import subprocess
import sys
import time
import uuid
from ctypes import wintypes
from pathlib import Path

try:
    from .native_windows import ExposureFailure, MonitorInfo, NativePointer, NativeWindows, has_mascot, observe_exposure, rgb_pixel, write_json
    from .composition import composite_decision, require_same_source, source_pixels
    from .move_entry_windows import observe_move_entry
except ImportError:
    from native_windows import ExposureFailure, MonitorInfo, NativePointer, NativeWindows, has_mascot, observe_exposure, rgb_pixel, write_json
    from composition import composite_decision, require_same_source, source_pixels
    from move_entry_windows import observe_move_entry


def static_source(executable: Path, output: Path, client, executable_sha256: str) -> tuple[bytes, dict[str, object]]:
    """Export one exact-build static frame before input/load measurement, using the observed physical scale."""
    if hashlib.sha256(executable.read_bytes()).hexdigest() != executable_sha256:
        raise RuntimeError("Native executable changed before exact-build static source export")
    root = output / "static-source"
    root.mkdir(exist_ok=False)
    temporary = root / "temporary"
    temporary.mkdir()
    scenario = [{"name": "static-first", "theme": "dark", "expanded": True, "scale": client.scale,
                 "flip": 0, "fill": 0, "sweep": 0, "pose": 0, "direction": 1,
                 "locale": "en", "activity": False, "raw": True}]
    definition = root / "scenario.json"
    write_json(definition, scenario)
    command = [str(executable), "--scenarios", str(definition), "--output", str(root)]
    environment = os.environ.copy()
    environment.update(TEMP=str(temporary), TMP=str(temporary))
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               env=environment, creationflags=0x08000000)
    try:
        stdout, stderr = process.communicate(timeout=30)
        if process.returncode != 0:
            raise RuntimeError(f"Static source export failed: exit {process.returncode}; stderr {stderr[-4096:].decode('utf-8', errors='replace')}")
    finally:
        if process.poll() is None:
            process.terminate()
            process.communicate(timeout=2)
    if hashlib.sha256(executable.read_bytes()).hexdigest() != executable_sha256:
        raise RuntimeError("Native executable changed during exact-build static source export")
    path = root / "static-first.raw"
    if path.stat().st_size != client.width * client.height * 4:
        raise RuntimeError("Static source export does not match the observed native client geometry")
    raw = path.read_bytes()
    source_pixels(raw, client.width, client.height)
    provenance = {"executable_sha256": executable_sha256, "export_command": command,
                  "export_pid": process.pid, "export_exit_code": process.returncode,
                  "export_stdout_bytes": len(stdout), "export_stderr_bytes": len(stderr),
                  "scenario_sha256": hashlib.sha256(definition.read_bytes()).hexdigest(),
                  "source_path": str(path), "source_sha256": hashlib.sha256(raw).hexdigest(),
                  "byte_count": len(raw), "width": client.width, "height": client.height,
                  "preparation": "Exact-build renderer export before physical gestures and controlled contention"}
    write_json(root / "provenance.json", provenance)
    return raw, provenance


def terminal_frame_audit(receipt: Path, source_path: Path, expected: bytes, width: int, height: int) -> dict[str, object]:
    """Verify the native audit of every successfully presented transition after owned teardown."""
    if receipt.stat().st_size > 65536:
        raise ValueError("Native terminal receipt exceeded its bounded metadata size")
    metadata = json.loads(receipt.read_text(encoding="utf-8"))
    audit = metadata.get("frame_audit")
    keys = {"width", "height", "presented_frames", "content_changes", "byte_count", "source_path"}
    if not isinstance(audit, dict) or set(audit) != keys:
        raise ValueError("Native terminal full-frame audit schema is missing or unrecognized")
    if any(type(audit[field]) is not int for field in keys - {"source_path"}):
        raise ValueError("Native frame audit requires integer geometry and transition counts")
    if (audit["width"], audit["height"], audit["byte_count"]) != (width, height, width * height * 4):
        raise ValueError("Native frame audit geometry differs from the admitted whole frame")
    if (audit["presented_frames"] < 1 or type(metadata.get("frames_presented")) is not int
            or audit["presented_frames"] != metadata["frames_presented"]):
        raise ValueError("Native frame audit did not cover every successfully presented frame")
    if audit["content_changes"] != 0:
        raise ValueError("Native full-source content changed during the static window lifetime")
    if not isinstance(audit["source_path"], str) or Path(audit["source_path"]).resolve(strict=True) != source_path.resolve(strict=True):
        raise ValueError("Native frame audit source is not the owned requested artifact")
    if source_path.stat().st_size != len(expected):
        raise ValueError("Native frame audit source byte count differs from the admitted full frame")
    actual = source_path.read_bytes()
    digest = require_same_source(actual, expected, width, height)
    return {**audit, "source_sha256": digest, "terminal_receipt_sha256": hashlib.sha256(receipt.read_bytes()).hexdigest(),
            "complete_source_immutable": True, "comparison": "Exact complete premultiplied RGBA bytes; every presented transition audited"}


def terminal_move_audit(receipt: Path, source_path: Path, width: int, height: int) -> dict[str, object]:
    """Require complete in-loop native submissions with multiple whole-source transitions after owned teardown."""
    if receipt.stat().st_size > 65536:
        raise ValueError("Native terminal receipt exceeded its bounded metadata size")
    metadata = json.loads(receipt.read_text(encoding="utf-8"))
    frame = metadata.get("frame_audit")
    frame_keys = {"width", "height", "presented_frames", "content_changes", "byte_count", "source_path"}
    if not isinstance(frame, dict) or set(frame) != frame_keys:
        raise ValueError("Native terminal full-frame audit schema is missing or unrecognized")
    if any(type(frame[key]) is not int for key in frame_keys - {"source_path"}):
        raise ValueError("Native frame audit requires integer geometry and transition counts")
    if (frame["width"], frame["height"], frame["byte_count"]) != (width, height, width * height * 4):
        raise ValueError("Native frame audit geometry differs from the admitted whole frame")
    if (type(metadata.get("frames_presented")) is not int or frame["presented_frames"] < 1
            or frame["presented_frames"] != metadata["frames_presented"]):
        raise ValueError("Native frame audit did not cover every successfully presented frame")
    if not 0 <= frame["content_changes"] < frame["presented_frames"]:
        raise ValueError("Native whole-source transition count exceeds its successful presentation inventory")
    if not isinstance(frame["source_path"], str) or Path(frame["source_path"]).resolve(strict=True) != source_path.resolve(strict=True):
        raise ValueError("Native frame audit source is not the owned requested artifact")
    if source_path.stat().st_size != width * height * 4:
        raise ValueError("Native first-source byte count differs from the admitted whole frame")
    raw = source_path.read_bytes()
    source_pixels(raw, width, height)
    audit = metadata.get("native_move_audit")
    if not isinstance(audit, dict) or set(audit) != {"schema_version", "move_frames_observed", "move_samples_omitted", "presentations"}:
        raise ValueError("Native moving-source audit schema is missing or unrecognized")
    if (type(audit["schema_version"]) is not int or audit["schema_version"] != 1
            or any(type(audit[key]) is not int or audit[key] < 0 for key in ("move_frames_observed", "move_samples_omitted"))):
        raise ValueError("Native moving-source audit version or counts are invalid")
    samples = audit["presentations"]
    if not isinstance(samples, list) or not 3 <= len(samples) <= 512:
        raise ValueError("Native moving-source audit requires three through 512 complete moving samples")
    if audit["move_samples_omitted"] != 0 or audit["move_frames_observed"] != len(samples):
        raise ValueError("Native moving-source audit omitted or miscounted in-loop presentations")
    for sample in samples:
        if not isinstance(sample, dict) or set(sample) != {"sequence", "elapsed_ms", "source_revision"}:
            raise ValueError("Native moving presentation schema is unrecognized")
        if (type(sample["sequence"]) is not int or not 1 <= sample["sequence"] <= frame["presented_frames"]
                or type(sample["source_revision"]) is not int
                or not 0 <= sample["source_revision"] <= min(frame["content_changes"], sample["sequence"] - 1)
                or type(sample["elapsed_ms"]) not in (int, float)
                or not math.isfinite(sample["elapsed_ms"]) or sample["elapsed_ms"] < 0):
            raise ValueError("Native moving presentation identity, revision or finite timestamp is invalid")
    pairs = list(zip(samples, samples[1:]))
    if any(a["sequence"] >= b["sequence"] or a["elapsed_ms"] >= b["elapsed_ms"]
           or a["source_revision"] > b["source_revision"] for a, b in pairs):
        raise ValueError("Native moving presentations are not ordered by sequence, time and source revision")
    if any(b["sequence"] != a["sequence"] + 1 for a, b in pairs):
        raise ValueError("Native moving presentations are not consecutive lifetime submissions; intervening sources are unobserved")
    transitions = sum(a["source_revision"] != b["source_revision"] for a, b in pairs)
    if transitions < 2:
        raise ValueError("Native source did not change between at least two consecutive moving presentation pairs")
    longest = metadata.get("longest_frame_interval_ms")
    if type(longest) not in (int, float) or not math.isfinite(longest) or longest < 0:
        raise ValueError("Native lifetime longest frame interval is missing or nonfinite")
    return {**audit, "whole_source_transitions_between_move_samples": transitions,
            "in_loop_submission_intervals": distribution([b["elapsed_ms"] - a["elapsed_ms"] for a, b in pairs]),
            "in_loop_sample_span_ms": samples[-1]["elapsed_ms"] - samples[0]["elapsed_ms"],
            "in_loop_first_submission_elapsed_ms": samples[0]["elapsed_ms"],
            "in_loop_last_submission_elapsed_ms": samples[-1]["elapsed_ms"],
            "lifetime_longest_frame_interval_ms": longest, "frame_audit": frame,
            "first_source_sha256": hashlib.sha256(raw).hexdigest(),
            "terminal_receipt_sha256": hashlib.sha256(receipt.read_bytes()).hexdigest(),
            "animated_source_activity_qualified": True,
            "observation": "Native owner observed the owned OS move loop before and after each successful audited presentation",
            "timing_scope": "Intervals between retained in-loop native submissions; compositor frame age and full entry/exit gaps are unmeasured"}


def distribution(samples: list[float]) -> dict[str, float | int]:
    """Summarize measured milliseconds by nearest-rank percentiles without invented budgets."""
    if not samples or any(not math.isfinite(value) or value < 0 for value in samples):
        raise ValueError("Latency distribution requires nonnegative finite observed samples")
    ordered = sorted(samples)
    return {"count": len(ordered), "minimum_ms": ordered[0], "maximum_ms": ordered[-1],
            **{f"p{percent}_ms": ordered[math.ceil(len(ordered) * percent / 100) - 1]
               for percent in (50, 95, 99)}}


def affinity(cpus: str) -> int:
    """Parse an explicit bounded logical-CPU set rather than guessing workstation affinity."""
    indices = [int(value) for value in cpus.split(",")]
    if not indices or len(indices) != len(set(indices)) or any(index < 0 or index >= min(64, os.cpu_count() or 1) for index in indices):
        raise ValueError("Affinity requires distinct available logical CPU indices below 64")
    return sum(1 << index for index in indices)


def error_causes(error: BaseException) -> list[dict[str, str]]:
    """Keep grouped boundary and cleanup causes reviewable in a failed receipt."""
    if isinstance(error, BaseExceptionGroup):
        return [cause for child in error.exceptions for cause in error_causes(child)]
    return [{"error_type": type(error).__name__, "error": str(error)}]


class OperationalOutput:
    """Preserve bounded native-proof diagnostic tails and the owned process's terminal shutdown phases."""

    limit = 65536

    def __init__(self, process: subprocess.Popen[bytes], output: Path) -> None:
        """Borrow only the Popen-owned native process and its private output pipes."""
        self.process, self.output = process, output
        self.stdout, self.stderr = b"", b""
        self.complete, self.forced = False, False
        self.phases: list[dict[str, object]] = []

    def collect(self, phase: str) -> None:
        """Drain owned pipes for at most five seconds, retaining partial evidence if shutdown fails."""
        event: dict[str, object] = {"phase": phase, "wait_bound_seconds": 5}
        self.phases.append(event)
        try:
            stdout, stderr = self.process.communicate(timeout=5)
        except subprocess.TimeoutExpired as error:
            if error.output is not None:
                self.stdout = error.output
            if error.stderr is not None:
                self.stderr = error.stderr
            event["causes"] = error_causes(error)
            raise
        except BaseException as error:
            event["causes"] = error_causes(error)
            raise
        if isinstance(stdout, bytes):
            self.stdout = stdout
        if isinstance(stderr, bytes):
            self.stderr = stderr
        if not isinstance(stdout, bytes) or not isinstance(stderr, bytes) or type(self.process.returncode) is not int:
            raise RuntimeError("Owned process drain did not observe complete byte streams and a terminal exit code")
        self.complete = True
        event["exit_code"] = self.process.returncode
        self.persist()

    def failed_teardown(self) -> None:
        """Terminate only the live owned process, then observe pipes/exit; preserve both failure causes."""
        errors: list[BaseException] = []
        if self.process.poll() is None:
            self.forced = True
            event: dict[str, object] = {"phase": "failed_scenario_owned_termination", "termination_requested": True}
            self.phases.append(event)
            try:
                self.process.terminate()
            except BaseException as error:
                event["causes"] = error_causes(error)
                errors.append(error)
        try:
            self.collect("failed_scenario_teardown")
        except BaseException as error:
            errors.append(error)
        if errors:
            raise BaseExceptionGroup("Owned operational process teardown failed", errors)

    def receipt(self) -> dict[str, object]:
        """Describe complete counts or partial lower bounds separately from bounded retained tails."""
        streams = {}
        for name, raw in (("stdout", self.stdout), ("stderr", self.stderr)):
            tail = raw[-self.limit:]
            streams[name] = {"bytes_observed": len(raw), "count_complete": self.complete,
                             "retained_tail_bytes": len(tail), "tail_truncated": len(raw) > self.limit,
                             "tail_path": str(self.output / f"{name}-tail.log"),
                             "tail_sha256": hashlib.sha256(tail).hexdigest()}
        return {"pid": self.process.pid, "exit_code": self.process.returncode,
                "streams_complete": self.complete, "forced_termination_requested": self.forced,
                "phases": self.phases, **streams,
                "scope": "Verified-owned Rust operational proof diagnostics; excludes authenticated Python reference output"}

    def persist(self) -> None:
        """Write at most 64 KiB per stream in the caller's owned artifact directory."""
        (self.output / "stdout-tail.log").write_bytes(self.stdout[-self.limit:])
        (self.output / "stderr-tail.log").write_bytes(self.stderr[-self.limit:])
        write_json(self.output / "process-output.json", self.receipt())


class OwnedContention:
    """Own fixed-lifetime qualification processes, readiness receipts and terminal cleanup."""

    def __init__(self, observer: ObservationWindows, workers: int, mask: int) -> None:
        """Keep explicit CPU affinity and a small admitted worker inventory."""
        if not 1 <= workers <= 8 or mask <= 0:
            raise ValueError("Admit one through eight workers with explicit nonzero affinity")
        self.observer, self.workers, self.mask = observer, workers, mask
        self.entries: list[dict[str, object]] = []

    def ready(self, process: subprocess.Popen[bytes], token: str) -> dict[str, object]:
        """Read only already-available pipe bytes; a missing or foreign handshake fails within one second."""
        import msvcrt
        handle = msvcrt.get_osfhandle(process.stdout.fileno())
        raw = bytearray()
        deadline = time.perf_counter() + 1
        while time.perf_counter() < deadline and process.poll() is None:
            available = wintypes.DWORD()
            if not self.observer.kernel.PeekNamedPipe(handle, None, 0, None, ctypes.byref(available), None):
                raise ctypes.WinError(ctypes.get_last_error())
            if available.value:
                if len(raw) + available.value > 4096:
                    raise RuntimeError("Owned load worker exceeded its readiness payload limit")
                raw.extend(process.stdout.read(available.value))
                if b"\n" in raw:
                    if time.perf_counter() >= deadline or process.poll() is not None:
                        raise RuntimeError("Owned load readiness arrived after its live observation boundary")
                    return self.validate_ready(bytes(raw), process.pid, token)
            time.sleep(.005)
        raise RuntimeError("Owned load worker did not acknowledge readiness within one second")

    def validate_ready(self, raw: bytes, pid: int, token: str) -> dict[str, object]:
        """Require the exact owned process, nonce, affinity and finite fixture lifetime."""
        record = json.loads(raw)
        if record != {"pid": pid, "token": token, "affinity_mask": self.mask, "maximum_lifetime_seconds": 15}:
            raise RuntimeError("Owned load worker readiness identity or affinity mismatch")
        return record

    def __enter__(self) -> OwnedContention:
        """Launch admitted workers and retain handles before any readiness operation can fail."""
        try:
            for _ in range(self.workers):
                token = uuid.uuid4().hex
                base = Path(sys._base_executable).resolve(strict=True)
                fixture = Path(__file__).with_name("contention_worker.py").resolve(strict=True)
                process = subprocess.Popen([str(base), str(fixture), "--token", token, "--affinity-mask", str(self.mask)],
                                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=0x08000000)
                entry: dict[str, object] = {"process": process}
                self.entries.append(entry)
                handle = self.observer.kernel.OpenProcess(0x1000 | 0x100000, False, process.pid)
                if not handle:
                    raise ctypes.WinError(ctypes.get_last_error())
                entry.update(handle=handle, before=self.observer.snapshot(handle), started=time.perf_counter())
                entry["ready"] = self.ready(process, token)
            return self
        except Exception as error:
            self.__exit__(type(error), error, error.__traceback__)
            raise

    def __exit__(self, kind: object, error: BaseException | None, traceback: object) -> bool:
        """Stop only owned subprocess handles, observe terminal outcomes and release all observation handles."""
        failures: list[Exception] = []
        for entry in self.entries:
            process = entry["process"]
            try:
                if "handle" in entry:
                    after = self.observer.snapshot(entry["handle"])
                    if after["creation_ticks"] != entry["before"]["creation_ticks"]:
                        raise RuntimeError("Owned load observation process identity changed")
                    elapsed = time.perf_counter() - entry["started"]
                    if elapsed <= 0:
                        raise RuntimeError("Owned load CPU measurement has no positive observation interval")
                    entry["measurement"] = {"pid": process.pid, "affinity_mask": self.mask,
                                            "elapsed_seconds": elapsed, "before": entry["before"], "after": after,
                                            "one_logical_core_percent": (after["cpu_100ns"] - entry["before"]["cpu_100ns"]) / 100000 / elapsed}
            except Exception as cleanup:
                failures.append(cleanup)
            try:
                if process.poll() is not None:
                    failures.append(RuntimeError("Owned load worker ended before the scenario completed"))
                else:
                    process.terminate()
                stdout, stderr = process.communicate(timeout=2)
                entry["terminal"] = {"exit_code": process.returncode, "stdout_bytes": len(stdout),
                                     "stderr_bytes": len(stderr), "stderr_diagnostic": stderr.decode(errors="replace")[:4096]}
            except Exception as cleanup:
                failures.append(cleanup)
            finally:
                if "handle" in entry and not self.observer.kernel.CloseHandle(entry["handle"]):
                    failures.append(RuntimeError("Owned load observation handle did not close"))
        if failures:
            if isinstance(error, Exception):
                failures.insert(0, error)
            raise ExceptionGroup("Owned contention and cleanup failures", failures)
        return False

    def receipt(self) -> list[dict[str, object]]:
        """Export observed load units and terminal outcomes, without process handles or nonce material."""
        return [{"measurement": entry.get("measurement"), "terminal": entry.get("terminal")} for entry in self.entries]


class ProcessMemory(ctypes.Structure):
    """Match PROCESS_MEMORY_COUNTERS on the observer's pointer-width ABI."""

    _fields_ = [("size", wintypes.DWORD), ("page_faults", wintypes.DWORD)] + [
        (name, ctypes.c_size_t) for name in (
            "peak_working_set", "working_set", "paged_peak", "paged", "nonpaged_peak",
            "nonpaged", "pagefile", "peak_pagefile",
        )
    ]


class GuiThreadInfo(ctypes.Structure):
    """Match GUITHREADINFO to observe actual caption move-loop entry and exit."""

    _fields_ = [("size", wintypes.DWORD), ("flags", wintypes.DWORD)] + [
        (name, wintypes.HWND) for name in ("active", "focus", "capture", "menu", "move", "caret")
    ] + [("caret_rect", wintypes.RECT)]


class ObservationWindows:
    """Borrow native resources for one verified process and close every owned observation handle."""

    def __init__(self, native: NativeWindows) -> None:
        """Declare ABI contracts before requesting process metrics or monitor operations."""
        self.native = native
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.psapi = ctypes.WinDLL("psapi", use_last_error=True)
        self.monitor_callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HANDLE, wintypes.HDC,
                                                   ctypes.POINTER(wintypes.RECT), wintypes.LPARAM)
        for function, arguments, result in (
            (self.kernel.OpenProcess, [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
            (self.kernel.CloseHandle, [wintypes.HANDLE], wintypes.BOOL),
            (self.kernel.GetProcessTimes, [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4, wintypes.BOOL),
            (self.kernel.WaitForSingleObject, [wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            (self.kernel.SetProcessAffinityMask, [wintypes.HANDLE, ctypes.c_size_t], wintypes.BOOL),
            (self.kernel.GetProcessAffinityMask, [wintypes.HANDLE, ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(ctypes.c_size_t)], wintypes.BOOL),
            (self.kernel.PeekNamedPipe, [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD),
                                         ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL),
            (self.psapi.GetProcessMemoryInfo, [wintypes.HANDLE, ctypes.POINTER(ProcessMemory), wintypes.DWORD], wintypes.BOOL),
            (native.user.GetGuiResources, [wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            (native.user.ShowWindow, [wintypes.HWND, ctypes.c_int], wintypes.BOOL),
            (native.user.IsWindowVisible, [wintypes.HWND], wintypes.BOOL),
            (native.user.EnumDisplayMonitors, [wintypes.HDC, ctypes.POINTER(wintypes.RECT), self.monitor_callback, wintypes.LPARAM], wintypes.BOOL),
            (native.user.SetWindowPos, [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.UINT], wintypes.BOOL),
            (native.user.MonitorFromWindow, [wintypes.HWND, wintypes.DWORD], wintypes.HANDLE),
            (native.user.GetGUIThreadInfo, [wintypes.DWORD, ctypes.POINTER(GuiThreadInfo)], wintypes.BOOL),
            (native.user.SendMessageTimeoutW, [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM,
                                              wintypes.UINT, wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)], ctypes.c_size_t),
        ):
            function.argtypes = arguments
            function.restype = result

    def set_affinity(self, hwnd: int, pid: int, mask: int) -> dict[str, int]:
        """Restrict only the owned fixture to explicit available CPUs, retaining its prior mask."""
        self.native.client(hwnd, pid)
        handle = self.kernel.OpenProcess(0x1000 | 0x200, False, pid)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            before, system = ctypes.c_size_t(), ctypes.c_size_t()
            if not self.kernel.GetProcessAffinityMask(handle, ctypes.byref(before), ctypes.byref(system)):
                raise ctypes.WinError(ctypes.get_last_error())
            if mask <= 0 or mask & ~system.value:
                raise RuntimeError("Explicit fixture affinity includes unavailable logical CPUs")
            if not self.kernel.SetProcessAffinityMask(handle, mask):
                raise ctypes.WinError(ctypes.get_last_error())
            return {"prior_mask": before.value, "observed_system_mask": system.value, "fixture_mask": mask}
        finally:
            if not self.kernel.CloseHandle(handle):
                raise ctypes.WinError(ctypes.get_last_error())

    def origin(self, hwnd: int, pid: int) -> tuple[int, int]:
        """Read the current owned window origin without changing placement."""
        self.native.client(hwnd, pid)
        rect = wintypes.RECT()
        if not self.native.user.GetWindowRect(hwnd, ctypes.byref(rect)):
            raise ctypes.WinError(ctypes.get_last_error())
        return rect.left, rect.top

    def response(self, hwnd: int, pid: int) -> float:
        """Measure synchronous WM_NULL delivery with a hard 250 ms boundary and ownership checks."""
        self.native.client(hwnd, pid)
        result = ctypes.c_size_t()
        started = time.perf_counter()
        if not self.native.user.SendMessageTimeoutW(hwnd, 0, 0, 0, 0x22, 250, ctypes.byref(result)):
            raise RuntimeError("Owned window did not respond within the 250 ms message boundary")
        elapsed = (time.perf_counter() - started) * 1000
        self.native.client(hwnd, pid)
        return elapsed

    def in_move_loop(self, hwnd: int, pid: int) -> bool:
        """Require the actual owned thread/window in its OS move loop, not posted keyboard intent."""
        self.native.client(hwnd, pid)
        identity = wintypes.DWORD()
        thread = self.native.user.GetWindowThreadProcessId(hwnd, ctypes.byref(identity))
        info = GuiThreadInfo()
        info.size = ctypes.sizeof(info)
        if not thread or identity.value != pid or not self.native.user.GetGUIThreadInfo(thread, ctypes.byref(info)):
            raise RuntimeError("Cannot observe the owned GUI thread move state")
        return bool(info.flags & 2) and info.move == hwnd

    def calibrate_drag(self, hwnd: int, pid: int, pointer, origin: tuple[int, int],
                       cursor: tuple[int, int], direction: tuple[int, int], deadline: float,
                       state: dict[str, object]) -> tuple[tuple[int, int], tuple[int, int]]:
        """Establish a causal cursor/window pair after queued horizontal entry input has been consumed."""
        state.update(before_origin=origin, before_cursor_screen=cursor, direction=direction, steps=[],
                     phase_bound_ms=750, measured_distribution_member=False)
        for index in (1, 2):
            started = time.perf_counter()
            phase_deadline = min(deadline, started + .75)
            target = (cursor[0] + direction[0] * 8, cursor[1] + direction[1] * 8)
            expected = (origin[0] + direction[0] * 8, origin[1] + direction[1] * 8) if index == 2 else None
            step = {"index": index, "cursor_screen_target": target, "expected_window_origin": expected,
                    "input_dispatched": False, "accepted": False, "observations": []}
            state["steps"].append(step)
            step["owned_move_loop_before"] = self.in_move_loop(hwnd, pid)
            if not step["owned_move_loop_before"]:
                raise RuntimeError("Owned OS move loop ended before drag calibration")
            if time.perf_counter() >= phase_deadline:
                raise RuntimeError("Drag calibration exceeded its phase or original ten-second deadline")
            pointer.move_screen(target)
            step["input_dispatched"] = True
            step["message_response_ms"] = self.response(hwnd, pid)
            while True:
                actual_cursor = pointer.cursor()
                current = self.origin(hwnd, pid)
                observed = time.perf_counter()
                step["observations"].append({"cursor_screen": actual_cursor, "window_origin": current,
                                               "after_dispatch_ms": (observed - started) * 1000})
                if max(abs(a - b) for a, b in zip(actual_cursor, target)) > 1:
                    raise RuntimeError("Actual cursor departed its owned drag calibration target")
                if observed >= phase_deadline:
                    raise RuntimeError("Drag calibration exceeded its phase or original ten-second deadline")
                arrived = (current[1] - origin[1]) * direction[1] > 2 if index == 1 else max(abs(a - b) for a, b in zip(current, expected)) <= 2
                if arrived:
                    break
                time.sleep(.005)
            step["owned_move_loop_after"] = self.in_move_loop(hwnd, pid)
            if not step["owned_move_loop_after"]:
                raise RuntimeError("Owned OS move loop ended during drag calibration")
            if time.perf_counter() >= phase_deadline:
                raise RuntimeError("Drag calibration exceeded its phase or original ten-second deadline")
            step.update(accepted=True, observed_window_origin=current, observed_cursor_screen=actual_cursor,
                        elapsed_ms=(observed - started) * 1000)
            origin, cursor = current, actual_cursor
        state.update(qualified_anchor_origin=origin, qualified_anchor_cursor_screen=cursor)
        return origin, cursor

    def mouse_drag(self, hwnd: int, pid: int, output: Path, samples: int, *, animated: bool = False,
                   expected_source: bytes | None = None) -> dict[str, object]:
        """Observe physical caption dragging with static equality or a two-second animated activity trace."""
        if not 8 <= samples <= 64:
            raise ValueError("Native drag admits eight through 64 physical movement samples")
        client = self.native.client(hwnd, pid)
        if not animated and expected_source is None:
            raise ValueError("Static drag requires the exact-build complete premultiplied source")
        initial = self.native.capture(hwnd, pid)
        if not has_mascot(initial, client.scale):
            raise RuntimeError("Static drag baseline is missing the mascot")
        baseline = initial.tobytes()
        initial_composite = composite_decision(expected_source, client.width, client.height, initial) if not animated else None
        if initial_composite is not None and not initial_composite["accepted"]:
            write_json(output / "drag-initial-composite.json", initial_composite)
            initial.save(output / "drag-initial-composite-failure.png")
            raise RuntimeError("Initial full captured frame violates its admitted premultiplied source-over bounds")
        initial.save(output / "drag-static-before.png")
        before = self.origin(hwnd, pid)
        monitor_handle = self.native.user.MonitorFromWindow(hwnd, 2)
        monitor = MonitorInfo()
        monitor.size = ctypes.sizeof(monitor)
        if not self.native.user.GetMonitorInfoW(monitor_handle, ctypes.byref(monitor)):
            raise ctypes.WinError(ctypes.get_last_error())
        distance_x, distance_y = samples * 4 + 16, samples * 2 + 16
        direction_x = 1 if before[0] + client.width + 64 + distance_x < monitor.work.right else -1
        direction_y = 1 if before[1] + client.height + distance_y < monitor.work.bottom else -1
        if (before[0] + client.width + 64 > monitor.work.right
                or before[0] - distance_x < monitor.work.left and direction_x < 0
                or before[1] - distance_y < monitor.work.top and direction_y < 0):
            raise RuntimeError("Owned work area has insufficient safe space for the bounded drag path")
        steps: list[dict[str, object]] = []
        states: dict[str, object] = {"before_origin": before, "move_loop_observations": []}
        try:
            with NativePointer(self.native, hwnd, pid) as pointer:
                pointer.move((250, 16))
                start_point = pointer.cursor()
                states["before_press"] = {"window_origin": self.origin(hwnd, pid), "cursor_screen": start_point}
                press_started = time.perf_counter()
                pointer.press()
                deadline = press_started + 10
                gesture: dict[str, object] = {}
                states["entry_gesture"] = gesture
                try:
                    observe_move_entry(self, hwnd, pid, pointer, start_point, press_started, gesture)
                finally:
                    states["move_loop_observations"] = gesture["observations"]
                anchor_origin, anchor_cursor = self.origin(hwnd, pid), pointer.cursor()
                states["move_loop_entry"] = {"window_origin": anchor_origin, "cursor_screen": anchor_cursor,
                                             "after_press_ms": (time.perf_counter() - press_started) * 1000}
                calibration: dict[str, object] = {}
                states["calibration"] = calibration
                anchor_origin, anchor_cursor = self.calibrate_drag(hwnd, pid, pointer, anchor_origin, anchor_cursor,
                                                                  (direction_x, direction_y), deadline, calibration)
                schedule_started = time.perf_counter()
                for index in range(1, samples + 1):
                    if animated:
                        target_time = schedule_started + index * 2 / samples
                        while (remaining := target_time - time.perf_counter()) > 0:
                            time.sleep(min(.005, remaining))
                    target = (anchor_cursor[0] + direction_x * index * 4, anchor_cursor[1] + direction_y * index * 2)
                    expected = (anchor_origin[0] + direction_x * index * 4, anchor_origin[1] + direction_y * index * 2)
                    latest = {"index": index, "cursor_screen_target": target, "expected_window_origin": expected,
                              "input_dispatched": False, "sample_accepted": False}
                    states["latest_movement"] = latest
                    latest["owned_move_loop_before_measurement"] = self.in_move_loop(hwnd, pid)
                    if not latest["owned_move_loop_before_measurement"]:
                        raise RuntimeError("Owned OS move loop ended before a physical movement measurement")
                    started = time.perf_counter()
                    pointer.move_screen(target)
                    latest["input_dispatched"] = True
                    response_ms = self.response(hwnd, pid)
                    latest["message_response_ms"] = response_ms
                    position_deadline = min(deadline, time.perf_counter() + .75)
                    while True:
                        current = self.origin(hwnd, pid)
                        latest["observed_window_origin"] = current
                        if max(abs(a - b) for a, b in zip(current, expected)) <= 2:
                            break
                        if time.perf_counter() >= position_deadline:
                            raise RuntimeError("Physical mouse movement did not move the owned window to its expected origin")
                        time.sleep(.005)
                    move_ms = (time.perf_counter() - started) * 1000
                    capture_started = time.perf_counter()
                    frame = self.native.capture(hwnd, pid)
                    frozen_pixels = frame.tobytes()
                    capture_ms = (time.perf_counter() - capture_started) * 1000
                    latest.update(frame_size=frame.size, frame_sha256=hashlib.sha256(frozen_pixels).hexdigest(),
                                  capture_ms=capture_ms, captured_rgb_equal_to_initial=frozen_pixels == baseline if not animated else None)
                    if frame.size != initial.size or not animated and not has_mascot(frame, client.scale):
                        raise RuntimeError("Physical drag lost valid owned client geometry or mascot paint")
                    composite = composite_decision(expected_source, client.width, client.height, frame) if not animated else None
                    if composite is not None and not composite["accepted"]:
                        latest["whole_frame_composite"] = composite
                        frame.save(output / "drag-static-mismatch.png")
                        raise RuntimeError("Full captured frame violates its retained premultiplied source-over bounds")
                    latest["owned_move_loop_after_measurement"] = self.in_move_loop(hwnd, pid)
                    if not latest["owned_move_loop_after_measurement"]:
                        raise RuntimeError("Owned OS move loop ended during a physical movement measurement")
                    steps.append({"index": index, "cursor_screen": target, "window_origin": current,
                                  "message_response_ms": response_ms, "move_observed_ms": move_ms,
                                  "capture_ms": capture_ms, "frame_sha256": hashlib.sha256(frozen_pixels).hexdigest(),
                                  "observed_at_seconds": time.perf_counter() - schedule_started,
                                  "progress_sha256": hashlib.sha256(frame.crop((round(24 * client.scale), round(356 * client.scale),
                                                                               round(534 * client.scale), round(362 * client.scale))).tobytes()).hexdigest(),
                                  "captured_rgb_equal_to_initial": frozen_pixels == baseline if not animated else None,
                                  "whole_frame_composite": composite,
                                  "owned_move_loop_before_measurement": True, "owned_move_loop_after_measurement": True})
                    latest["sample_accepted"] = True
                    if time.perf_counter() >= deadline:
                        raise RuntimeError("Physical drag exceeded its ten-second observation boundary")
                pointer.release()
                exit_deadline = time.perf_counter() + 1
                while self.in_move_loop(hwnd, pid):
                    if time.perf_counter() >= exit_deadline:
                        raise RuntimeError("Owned OS move loop did not end after physical release")
                    time.sleep(.005)
            progress_changes = sum(a["progress_sha256"] != b["progress_sha256"] for a, b in zip(steps, steps[1:]))
            return {"mode": "SendInput native mouse caption drag", "physical_mouse_drag_qualified": True,
                    "before_origin": before, "after_origin": self.origin(hwnd, pid), "samples": steps,
                    "entry_gesture": states["entry_gesture"],
                    "calibration": states["calibration"],
                    "move_loop_entry": states["move_loop_entry"],
                    "static_source_immutability": "Requires terminal native full-source audit before acceptance" if not animated else None,
                    "initial_whole_frame_composite": initial_composite,
                    "frame_observation": "PrintWindow owned client RGB including variable native material; every pixel checked against full premultiplied foreground bounds" if not animated else "PrintWindow owned client RGB activity samples",
                    "capture_limit": "Synchronous PrintWindow is not an interruptible stall watchdog; timing describes returning captures",
                    "message_response": distribution([step["message_response_ms"] for step in steps]),
                    "move_observation": distribution([step["move_observed_ms"] for step in steps]),
                    "capture_observation": distribution([step["capture_ms"] for step in steps]),
                    "captured_progress_rgb_changes_observed": progress_changes if animated else None,
                    "animated_source_activity_qualified": False,
                    "animated_source_activity": "Requires terminal in-loop native whole-source transitions" if animated else None,
                    "frame_age_limit": "Captured RGB includes variable native material; compositor frame age is unmeasured"}
        except BaseException as error:
            states["failure_causes"] = error_causes(error)
            raise
        finally:
            write_json(output / "drag-state.json", states)
            write_json(output / "drag-samples.json", steps)

    def input_latency(self, hwnd: int, pid: int, output: Path, samples: int) -> dict[str, object]:
        """Measure actual caption clicks to visible compositor disclosure changes with causal samples."""
        if not 8 <= samples <= 64:
            raise ValueError("Input latency admits eight through 64 actual caption gestures")
        observations: list[dict[str, object]] = []
        try:
            with NativePointer(self.native, hwnd, pid) as pointer:
                pointer.move((489, 16))
                for index in range(samples):
                    client = self.native.client(hwnd, pid)
                    before = rgb_pixel(self.native.capture(hwnd, pid, compositor=True),
                                       (round(30 * client.scale), round(400 * client.scale)))
                    started = time.perf_counter()
                    pointer.press()
                    pointer.release()
                    deadline = started + 1
                    while True:
                        frame = self.native.capture(hwnd, pid, compositor=True)
                        after = rgb_pixel(frame, (round(30 * client.scale), round(400 * client.scale)))
                        observed = time.perf_counter()
                        if observed >= deadline:
                            raise RuntimeError("Actual caption click did not visibly respond within one second")
                        if max(abs(a - b) for a, b in zip(before, after)) > 5:
                            break
                        time.sleep(.005)
                    observations.append({"index": index, "input_to_visible_ms": (observed - started) * 1000,
                                         "before_rgb": before, "after_rgb": after,
                                         "frame_sha256": hashlib.sha256(frame.tobytes()).hexdigest()})
            return {"mode": "Actual SendInput caption disclosure response", "samples": observations,
                    "input_to_visible": distribution([sample["input_to_visible_ms"] for sample in observations]),
                    "measurement_scope": "OS mouse insertion through owned unoccluded compositor pixels; includes capture sampling cost"}
        finally:
            write_json(output / "input-latency-samples.json", observations)

    def snapshot(self, handle: int) -> dict[str, int]:
        """Observe CPU ticks, resident bytes and native object counts from the same owned process."""
        timestamps = [wintypes.FILETIME() for _ in range(4)]
        if not self.kernel.GetProcessTimes(handle, *(ctypes.byref(value) for value in timestamps)):
            raise ctypes.WinError(ctypes.get_last_error())
        ticks = lambda value: (value.dwHighDateTime << 32) | value.dwLowDateTime
        memory = ProcessMemory()
        memory.size = ctypes.sizeof(memory)
        if not self.psapi.GetProcessMemoryInfo(handle, ctypes.byref(memory), memory.size):
            raise ctypes.WinError(ctypes.get_last_error())
        return {"creation_ticks": ticks(timestamps[0]),
                "cpu_100ns": ticks(timestamps[2]) + ticks(timestamps[3]),
                "working_set_bytes": memory.working_set,
                "gdi_objects": self.native.user.GetGuiResources(handle, 0),
                "user_objects": self.native.user.GetGuiResources(handle, 1)}

    def cpu(self, hwnd: int, pid: int, visibility: str) -> dict[str, object]:
        """Measure an explicit 30-second static interval, then require restored character paint."""
        self.native.client(hwnd, pid)
        handle = self.kernel.OpenProcess(0x1000 | 0x100000, False, pid)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            if visibility != "visible":
                self.native.user.ShowWindow(hwnd, 0 if visibility == "hidden" else 6)
                if visibility == "hidden" and self.native.user.IsWindowVisible(hwnd):
                    raise RuntimeError("Owned static window did not become hidden")
                if visibility == "minimized" and not self.native.user.IsIconic(hwnd):
                    raise RuntimeError("Owned static window did not become minimized")
            before = self.snapshot(handle)
            started = time.perf_counter()
            outcome = self.kernel.WaitForSingleObject(handle, 30000)
            elapsed = time.perf_counter() - started
            if outcome != 0x102:
                raise RuntimeError(f"Owned static process ended or observation failed: wait status {outcome}")
            after = self.snapshot(handle)
            if before["creation_ticks"] != after["creation_ticks"]:
                raise RuntimeError("Owned process identity changed during CPU observation")
            one_core_percent = (after["cpu_100ns"] - before["cpu_100ns"]) / 10_000_000 / elapsed * 100
            self.native.client(hwnd, pid)
            self.native.user.ShowWindow(hwnd, 9)
            deadline = time.perf_counter() + 2
            while True:
                image = self.native.capture(hwnd, pid)
                if has_mascot(image, self.native.client(hwnd, pid).scale):
                    break
                if time.perf_counter() >= deadline:
                    raise RuntimeError("Owned static window did not restore character paint")
                time.sleep(0.005)
            return {"visibility": visibility, "interval_seconds": elapsed,
                    "one_logical_core_percent": one_core_percent, "below_one_percent": one_core_percent < 1,
                    "before": before, "after": after, "restore_paint_observed": True,
                    "scope": "S0 demonstration process; no supervisor/backend/browser aggregation claim"}
        finally:
            if not self.kernel.CloseHandle(handle):
                raise ctypes.WinError(ctypes.get_last_error())

    def monitors(self) -> list[tuple[int, MonitorInfo]]:
        """Enumerate actual monitors without changing display configuration or DPI."""
        monitors = []

        def visit(handle: int, _: int, rect: object, data: int) -> bool:
            info = MonitorInfo()
            info.size = ctypes.sizeof(info)
            if not self.native.user.GetMonitorInfoW(handle, ctypes.byref(info)):
                return False
            monitors.append((handle, info))
            return len(monitors) < 16

        callback = self.monitor_callback(visit)
        if not self.native.user.EnumDisplayMonitors(None, None, callback, 0):
            raise ctypes.WinError(ctypes.get_last_error())
        return monitors

    def dpi(self, hwnd: int, pid: int) -> dict[str, object]:
        """Move the owned window through real work areas and require observed logical geometry."""
        observations = []
        for handle, monitor in self.monitors():
            client = self.native.client(hwnd, pid)
            work = monitor.work
            x = work.left + (work.right - work.left - client.width) // 2
            y = work.top + (work.bottom - work.top - client.height) // 2
            if not self.native.user.SetWindowPos(hwnd, None, x, y, 0, 0, 0x15):
                raise ctypes.WinError(ctypes.get_last_error())
            deadline = time.perf_counter() + 2
            while True:
                current = self.native.client(hwnd, pid)
                expected = math.floor(558 * current.scale + 0.5)
                if current.width == current.height == expected and self.native.user.MonitorFromWindow(hwnd, 2) == handle:
                    break
                if time.perf_counter() >= deadline:
                    raise RuntimeError("Real monitor DPI transition did not preserve 558 logical pixels")
                time.sleep(0.005)
            observations.append({"monitor_handle": handle, "work_area": [work.left, work.top, work.right, work.bottom],
                                 "dpi": current.dpi, "client_device_size": [current.width, current.height]})
        changed = len({observation["dpi"] for observation in observations}) > 1
        return {"mixed_dpi_status": "observed" if changed else "pending_unavailable_mixed_dpi",
                "monitors": observations, "synthetic_dpi_messages": False}

    def move_loop(self, hwnd: int, pid: int) -> dict[str, object]:
        """Exercise the real OS modal move loop through keyboard input; this is not physical dragging."""
        self.native.client(hwnd, pid)
        before = wintypes.RECT()
        if not self.native.user.GetWindowRect(hwnd, ctypes.byref(before)):
            raise ctypes.WinError(ctypes.get_last_error())
        self.native.post(hwnd, pid, 0x0112, 0xF012, 0)
        for key in (39, 39, 40):
            self.native.post(hwnd, pid, 0x0100, key, 1)
            self.native.post(hwnd, pid, 0x0101, key, 0xC0000001)
        self.native.post(hwnd, pid, 0x0100, 13, 1)
        self.native.post(hwnd, pid, 0x0101, 13, 0xC0000001)
        deadline = time.perf_counter() + 2
        while True:
            self.native.client(hwnd, pid)
            after = wintypes.RECT()
            if not self.native.user.GetWindowRect(hwnd, ctypes.byref(after)):
                raise ctypes.WinError(ctypes.get_last_error())
            if (before.left, before.top) != (after.left, after.top):
                break
            if time.perf_counter() >= deadline:
                raise RuntimeError("Owned OS caption move loop did not change the window origin")
            time.sleep(0.005)
        result = ctypes.c_size_t()
        started = time.perf_counter()
        if not self.native.user.SendMessageTimeoutW(hwnd, 0, 0, 0, 2, 250, ctypes.byref(result)):
            raise RuntimeError("Owned window did not respond after its OS move loop")
        return {"mode": "OS keyboard move loop", "physical_mouse_drag_qualified": False,
                "before_origin": [before.left, before.top], "after_origin": [after.left, after.top],
                "response_ms": (time.perf_counter() - started) * 1000}


def main() -> int:
    """Launch one owned static demonstration and retain distinct native operational receipts."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--mode", choices=("cpu", "dpi", "move-loop", "mouse-drag", "input-latency"))
    parser.add_argument("--visibility", default="visible", choices=("visible", "hidden", "minimized"))
    parser.add_argument("--samples", default=32, type=int)
    parser.add_argument("--animated", action="store_true")
    parser.add_argument("--workers", default=0, type=int)
    parser.add_argument("--affinity-cpus")
    args = parser.parse_args()
    if args.executable is None or args.output is None or args.mode is None:
        parser.error("Native qualification requires --executable, --output and --mode")
    if not 0 <= args.workers <= 8 or not 8 <= args.samples <= 64:
        parser.error("Admit zero through eight workers and eight through 64 native samples")
    if args.workers and (args.affinity_cpus is None or args.mode not in ("mouse-drag", "input-latency")):
        parser.error("Controlled load requires explicit --affinity-cpus and drag or input-latency mode")
    if args.animated and args.mode != "mouse-drag":
        parser.error("Animated observation is the separate mouse-drag scenario")
    mask = affinity(args.affinity_cpus) if args.affinity_cpus is not None else None
    args.output.mkdir(parents=True, exist_ok=False)
    executable = args.executable.resolve(strict=True)
    executable_sha256 = hashlib.sha256(executable.read_bytes()).hexdigest()
    native = NativeWindows()
    observer = ObservationWindows(native)
    command = [str(executable), "--duration-ms", "45000", "--receipt", str(args.output / "submission.json")]
    if not args.animated:
        command.append("--reduced-motion")
    audited_static_drag = args.mode == "mouse-drag" and not args.animated
    audit_source = args.output / "audit-first.raw"
    if args.mode == "mouse-drag":
        command.extend(("--frame-audit", str(audit_source)))
    process = subprocess.Popen(command,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=0x08000000)
    output = OperationalOutput(process, args.output)
    primary_error: BaseException | None = None
    try:
        hwnd = native.find_window(process, time.perf_counter() + 5)
        deadline = time.perf_counter() + 5
        while not has_mascot(native.capture(hwnd, process.pid), native.client(hwnd, process.pid).scale):
            if time.perf_counter() >= deadline:
                raise RuntimeError("Operational fixture did not display the owned mascot")
            time.sleep(0.005)
        exposure = None
        if args.mode in ("mouse-drag", "input-latency", "move-loop"):
            exposure_started = time.perf_counter()
            exposed = observe_exposure(native, hwnd, process, exposure_started + 5)
            exposure = {"wait_ms": (exposed - exposure_started) * 1000,
                        "state": native.exposure_state(hwnd, process.pid)}
        affinity_receipt = observer.set_affinity(hwnd, process.pid, mask) if mask is not None else None
        expected, source_provenance = None, None
        source_client = native.client(hwnd, process.pid)
        if audited_static_drag:
            expected, source_provenance = static_source(executable, args.output, source_client, executable_sha256)

        def scenario() -> dict[str, object]:
            if args.mode == "cpu":
                return observer.cpu(hwnd, process.pid, args.visibility)
            if args.mode == "dpi":
                return observer.dpi(hwnd, process.pid)
            if args.mode == "move-loop":
                return observer.move_loop(hwnd, process.pid)
            if args.mode == "input-latency":
                return observer.input_latency(hwnd, process.pid, args.output, args.samples)
            return observer.mouse_drag(hwnd, process.pid, args.output, args.samples, animated=args.animated,
                                       expected_source=expected)

        if args.workers:
            load = OwnedContention(observer, args.workers, mask)
            try:
                with load:
                    result = scenario()
            finally:
                write_json(args.output / "contention.json", load.receipt())
        else:
            result = scenario()
        result.update(affinity=affinity_receipt, contention_workers=args.workers, initial_exposure=exposure,
                      workload="Fixed integer CPU fixture; observed units recorded separately" if args.workers else "uncontended fixture")
        result.update(pid=process.pid, executable_sha256=executable_sha256, static_source_provenance=source_provenance)
        native.post(hwnd, process.pid, 0x0010, 0, 0)
        output.collect("graceful_close")
        if process.returncode != 0:
            raise RuntimeError(f"Owned operational fixture exited with status {process.returncode}")
        if audited_static_drag:
            result["frame_audit"] = terminal_frame_audit(args.output / "submission.json", audit_source, expected,
                                                         source_client.width, source_client.height)
            result["static_immutable_pixels"] = True
        if args.mode == "mouse-drag" and args.animated:
            result["native_move_audit"] = terminal_move_audit(args.output / "submission.json", audit_source,
                                                            source_client.width, source_client.height)
            result["animated_source_activity_qualified"] = True
        result["process_output"] = output.receipt()
        write_json(args.output / "observation.json", result)
        return 1 if args.mode == "cpu" and not result["below_one_percent"] else 0
    except BaseException as error:
        primary_error = error
        raise
    finally:
        cleanup_errors: list[BaseException] = []
        if not output.complete:
            try:
                output.failed_teardown()
            except BaseException as error:
                cleanup_errors.append(error)
        if primary_error is not None or cleanup_errors:
            combined = ([primary_error] if primary_error is not None else []) + cleanup_errors
            failure = {"pid": process.pid, "error": str(primary_error) if primary_error else "Owned process cleanup failed",
                       "causes": [cause for error in combined for cause in error_causes(error)],
                       "process_output": output.receipt(), "qualification_accepted": False}
            if isinstance(primary_error, ExposureFailure):
                failure["exposure_state"] = primary_error.state
            try:
                output.persist()
                write_json(args.output / "failure.json", failure)
            except BaseException as error:
                cleanup_errors.append(error)
            if cleanup_errors:
                raise BaseExceptionGroup("Operational observation and owned cleanup failed",
                                         ([primary_error] if primary_error is not None else []) + cleanup_errors)


if __name__ == "__main__":
    raise SystemExit(main())
