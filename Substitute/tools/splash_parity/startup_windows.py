# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Measure Rust and the production PySide splash through the same external Windows observer."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path
from ctypes import wintypes

try:
    from .native_windows import ExposureFailure, NativePointer, NativeWindows, observe_compositor, rgb_pixel, write_json
    from .native_decisions import startup_decision
    from .timing_policy import TIMING_PROFILES, assess_timing
    from .foreground_launcher import ForegroundLauncher
except ImportError:
    from native_windows import ExposureFailure, NativePointer, NativeWindows, observe_compositor, rgb_pixel, write_json
    from native_decisions import startup_decision
    from timing_policy import TIMING_PROFILES, assess_timing
    from foreground_launcher import ForegroundLauncher


class ProcessEntry(ctypes.Structure):
    """Match PROCESSENTRY32W, retaining parent identity without reading command lines."""

    _fields_ = [("size", wintypes.DWORD), ("usage", wintypes.DWORD), ("pid", wintypes.DWORD),
                ("heap", ctypes.c_size_t), ("module", wintypes.DWORD), ("threads", wintypes.DWORD),
                ("parent", wintypes.DWORD), ("priority", wintypes.LONG), ("flags", wintypes.DWORD),
                ("executable", wintypes.WCHAR * 260)]


class ReferenceProcesses:
    """Borrow the launcher handle and retain verified direct interpreter children by native handle."""

    def __init__(self) -> None:
        """Declare pointer-width process APIs before ancestry, identity and lifetime observation."""
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.native = ctypes.WinDLL("ntdll")
        self.native.NtSuspendProcess.argtypes = [wintypes.HANDLE]
        self.native.NtSuspendProcess.restype = wintypes.LONG
        for function, arguments, result in (
            (self.kernel.CreateToolhelp32Snapshot, [wintypes.DWORD, wintypes.DWORD], wintypes.HANDLE),
            (self.kernel.Process32FirstW, [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)], wintypes.BOOL),
            (self.kernel.Process32NextW, [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)], wintypes.BOOL),
            (self.kernel.OpenProcess, [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
            (self.kernel.GetProcessId, [wintypes.HANDLE], wintypes.DWORD),
            (self.kernel.QueryFullProcessImageNameW, [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL),
            (self.kernel.GetProcessTimes, [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4, wintypes.BOOL),
            (self.kernel.WaitForSingleObject, [wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            (self.kernel.TerminateProcess, [wintypes.HANDLE, wintypes.UINT], wintypes.BOOL),
            (self.kernel.CloseHandle, [wintypes.HANDLE], wintypes.BOOL),
        ):
            function.argtypes, function.restype = arguments, result

    def close(self, handle: int) -> None:
        """Release only an owned child or snapshot handle."""
        if not self.kernel.CloseHandle(handle):
            raise ctypes.WinError(ctypes.get_last_error())

    def lifetime_identity(self, handle: int) -> tuple[int, int]:
        """Read PID and creation100ns from a caller-retained process object, including after exit."""
        if type(handle) is not int or not 0 < handle < ctypes.c_void_p(-1).value:
            raise ValueError("Process identity requires a retained positive native handle")
        pid = self.kernel.GetProcessId(handle)
        if not pid:
            raise ctypes.WinError(ctypes.get_last_error())
        if type(pid) is not int or not 0 < pid <= 0xFFFFFFFF:
            raise RuntimeError("Retained process PID has invalid native metadata")
        timestamps = [wintypes.FILETIME() for _ in range(4)]
        if not self.kernel.GetProcessTimes(handle, *(ctypes.byref(value) for value in timestamps)):
            raise ctypes.WinError(ctypes.get_last_error())
        creation = (timestamps[0].dwHighDateTime << 32) | timestamps[0].dwLowDateTime
        if creation == 0:
            raise RuntimeError("Retained process creation time is not positive")
        return pid, creation

    def identity(self, handle: int) -> tuple[int, Path, int]:
        """Read the live executable path together with the exact retained lifetime identity."""
        pid, creation = self.lifetime_identity(handle)
        path = ctypes.create_unicode_buffer(32768)
        capacity = wintypes.DWORD(len(path))
        if not self.kernel.QueryFullProcessImageNameW(handle, 0, path, ctypes.byref(capacity)):
            raise ctypes.WinError(ctypes.get_last_error())
        return pid, Path(path.value).resolve(strict=True), creation

    def ancestry(self) -> dict[int, list[tuple[int, str]]]:
        """Group one bounded native snapshot by parent for a complete discovery pass."""
        snapshot = self.kernel.CreateToolhelp32Snapshot(2, 0)
        if snapshot in (None, 0, ctypes.c_void_p(-1).value):
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            entry = ProcessEntry()
            entry.size = ctypes.sizeof(entry)
            found = self.kernel.Process32FirstW(snapshot, ctypes.byref(entry))
            children = {}
            count = 0
            while found:
                count += 1
                if count > 4096:
                    raise RuntimeError("Reference ancestry snapshot exceeded its process inventory bound")
                children.setdefault(entry.parent, []).append((entry.pid, entry.executable))
                found = self.kernel.Process32NextW(snapshot, ctypes.byref(entry))
            if ctypes.get_last_error() != 18:
                raise ctypes.WinError(ctypes.get_last_error())
            return children
        finally:
            self.close(snapshot)

    def open(self, pid: int) -> int:
        """Retain query, synchronization and termination rights for one candidate direct child."""
        handle = self.kernel.OpenProcess(0x1000 | 0x100000 | 1, False, pid)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        return handle

    def alive(self, handle: int) -> bool:
        """Test liveness on a retained process object without adopting a reused PID."""
        return not self.wait_exited(handle, 0)

    def wait_exited(self, handle: int, timeout_ms: int) -> bool:
        """Observe the original process's exit event with a bounded wait of at most two seconds."""
        if type(timeout_ms) is not int or not 0 <= timeout_ms <= 2000:
            raise ValueError("Process exit observation requires a bounded millisecond timeout")
        status = self.kernel.WaitForSingleObject(handle, timeout_ms)
        if status not in (0, 0x102):
            raise RuntimeError(f"Reference process observation failed with wait status {status}")
        return status == 0

    def stop(self, handle: int) -> None:
        """Terminate only a verified owned interpreter, then observe its terminal state within two seconds."""
        if self.alive(handle):
            if not self.kernel.TerminateProcess(handle, 1):
                raise ctypes.WinError(ctypes.get_last_error())
            if not self.wait_exited(handle, 2000):
                raise RuntimeError("The owned reference interpreter did not terminate within two seconds")

    def suspend(self, handle: int) -> None:
        """Fault the retained process through a separately verified, minimally privileged handle.

        The original handle remains owned throughout; querying and matching creation identity
        prevents fault injection from adopting a different process through a numeric PID.
        """
        original = self.lifetime_identity(handle)
        suspension = self.kernel.OpenProcess(0x1000 | 0x0800, False, original[0])
        if not suspension:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            if self.lifetime_identity(suspension) != original:
                raise RuntimeError("Suspension process differs from retained custody")
            status = self.native.NtSuspendProcess(suspension)
            if status != 0:
                raise RuntimeError(f"Owned root suspension failed: {status:#x}")
        finally:
            self.close(suspension)


class ReferenceOwner:
    """Keep the trusted venv launcher and its exact base-interpreter window process as one fixture lifetime."""

    def __init__(self, process: subprocess.Popen[bytes], launcher: Path, base: Path,
                 processes: ReferenceProcesses) -> None:
        """Borrow CPython Popen's native launcher handle; it remains owned and closed by Popen."""
        self.process, self.launcher, self.base, self.processes = process, launcher, base, processes
        self.pid = process.pid
        self.handle: int | None = None
        self.metadata: dict[str, object] = {}
        self.parent_handle = int(process._handle)
        pid, path, self.parent_creation = processes.identity(self.parent_handle)
        if pid != process.pid or os.path.normcase(str(path)) != os.path.normcase(str(launcher)):
            raise RuntimeError("Reference launcher native handle does not match the trusted interpreter")

    def bind(self, deadline: float) -> None:
        """Adopt exactly one direct child with matching executable and ordered native creation time."""
        while time.perf_counter() < deadline and self.process.poll() is None:
            candidates = []
            try:
                for pid, name in self.processes.ancestry().get(self.process.pid, ()):
                    if name.casefold() != self.base.name.casefold():
                        continue
                    handle = self.processes.open(pid)
                    candidates.append(handle)
                    actual_pid, path, creation = self.processes.identity(handle)
                    if actual_pid != pid or os.path.normcase(str(path)) != os.path.normcase(str(self.base)):
                        raise RuntimeError("Reference child does not match the trusted base interpreter")
                    if creation < self.parent_creation or not self.processes.alive(handle):
                        raise RuntimeError("Reference child creation/liveness does not match the owned launcher")
                if len(candidates) > 1:
                    raise RuntimeError("Reference launcher has multiple matching interpreter children")
                if candidates:
                    if self.processes.identity(self.parent_handle)[2] != self.parent_creation or self.process.poll() is not None:
                        raise RuntimeError("Reference launcher identity/lifetime changed during child admission")
                    if time.perf_counter() >= deadline:
                        raise RuntimeError("Reference child admission exceeded the original launch deadline")
                    handle = candidates[0]
                    actual_pid, _, creation = self.processes.identity(handle)
                    self.pid = actual_pid
                    self.handle = handle
                    candidates.remove(handle)
                    self.metadata = {"launcher_pid": self.process.pid, "launcher_creation_100ns": self.parent_creation,
                                     "window_pid": self.pid, "window_creation_100ns": creation,
                                     "trusted_base_executable": str(self.base), "relationship": "verified direct interpreter child"}
                    return
            finally:
                for handle in candidates:
                    self.processes.close(handle)
            time.sleep(.005)
        raise RuntimeError("Trusted reference interpreter child was not admitted within the launch deadline")

    def poll(self) -> int | None:
        """Treat either owned role's termination as the end of live window qualification."""
        parent = self.process.poll()
        if parent is not None:
            return parent
        if self.handle is not None and not self.processes.alive(self.handle):
            return 0
        return None

    def close(self) -> None:
        """Stop and close only the verified child, while preserving terminal-cleanup failures."""
        if self.handle is not None:
            handle, self.handle = self.handle, None
            failures = []
            try:
                self.processes.stop(handle)
            except Exception as error:
                failures.append(error)
            try:
                self.processes.close(handle)
            except Exception as error:
                failures.append(error)
            if failures:
                raise ExceptionGroup("Verified reference child cleanup failed", failures)


def reference_base(interpreter: Path, environment: dict[str, str]) -> Path:
    """Probe the trusted venv before timing to identify its actual installed base interpreter."""
    probe = subprocess.run([str(interpreter), "-c", "import json,sys; print(json.dumps({'base_executable':sys._base_executable}))"],
                           env=environment, capture_output=True, check=False, timeout=5)
    if probe.returncode != 0 or len(probe.stdout) > 4096:
        raise RuntimeError("Trusted reference interpreter could not identify its bounded base executable")
    value = json.loads(probe.stdout)
    if set(value) != {"base_executable"} or not isinstance(value["base_executable"], str):
        raise RuntimeError("Reference base-executable probe returned invalid metadata")
    base = Path(value["base_executable"])
    if not base.is_absolute():
        raise RuntimeError("Reference base executable is not an absolute trusted path")
    return base.resolve(strict=True)


def launch(arguments: argparse.Namespace, output: Path) -> tuple[list[str], Path, dict[str, str]]:
    """Use actual production entrypoints, preserving interpreter and font/cache state."""
    if arguments.timing_audit and arguments.mode != "rust":
        raise ValueError("Native timing diagnostics require the owned Rust proof")
    environment = os.environ.copy()
    temporary = arguments.temporary.resolve(strict=True)
    environment.update(TEMP=str(temporary), TMP=str(temporary), PYTHONDONTWRITEBYTECODE="1")
    # A preceding offscreen capture must not turn native qualification into a hidden Qt run.
    environment.pop("QT_QPA_PLATFORM", None)
    if arguments.mode == "reference":
        root = arguments.legacy_root.resolve(strict=True)
        interpreter = arguments.interpreter.resolve(strict=True)
        command = [str(interpreter), "-m", "substitute.app.bootstrap.shared_splash_host",
                   "--maximum-lifetime-seconds", str(arguments.duration_ms / 1000),
                   "--theme-mode", "light" if arguments.light else "dark",
                   "--backdrop-mode", "mica", "--locale", arguments.locale]
        return command, root, environment
    executable = arguments.executable.resolve(strict=True)
    command = [str(executable), "--duration-ms", str(arguments.duration_ms), "--collapsed",
               "--receipt", str(output / "submission.json"), "--locale", arguments.locale]
    if arguments.light:
        command.append("--light")
    if arguments.timing_audit:
        command.append("--timing-audit")
    return command, executable.parent, environment


def retain_diagnostics(arguments: argparse.Namespace, output: Path,
                       stdout: bytes, stderr: bytes) -> dict[str, object]:
    """Retain bounded Rust proof diagnostics after observed termination; reference output stays private."""
    if arguments.mode != "rust" or not arguments.timing_audit:
        raise ValueError("Diagnostic output is restricted to the opted-in owned Rust proof")
    streams = {}
    for name, raw in (("stdout", stdout), ("stderr", stderr)):
        tail = raw[-65536:]
        path = output / f"{name}-tail.log"
        path.write_bytes(tail)
        streams[name] = dict(bytes_observed=len(raw), retained_tail_bytes=len(tail),
                             tail_truncated=len(raw) > len(tail), tail_path=str(path),
                             tail_sha256=hashlib.sha256(tail).hexdigest())
    return streams


def observe(arguments: argparse.Namespace, output: Path, ordinal: int,
            native: NativeWindows) -> dict[str, object]:
    """Observe compositor paint and a causal caption response before writing bulky images."""
    assess_timing(None, 1, profile=arguments.timing_profile)
    output.mkdir(parents=True, exist_ok=False)
    command, cwd, environment = launch(arguments, output)
    receipt: dict[str, object] = {
        "mode": arguments.mode, "run_ordinal": ordinal,
        "run_class": "first_in_series" if ordinal == 0 else "warm_repeat",
        "cache_policy": "Existing font and application caches retained; no cold-cache claim",
        "command": command, "cwd": str(cwd),
        "creation_flags": 0x08000000,
        "creation_policy": "CREATE_NO_WINDOW matches the production Windows splash process owner; native GUI remains visible",
        "entrypoint_sha256": hashlib.sha256(Path(command[0]).read_bytes()).hexdigest(),
    }
    if arguments.mode == "reference":
        base = reference_base(Path(command[0]), environment)
        receipt["reference_base_executable_sha256"] = hashlib.sha256(base.read_bytes()).hexdigest()
        receipt["interpreter_probe_policy"] = "Base executable metadata resolved before the launch timer; caches retained"
        receipt["reference_source_sha256"] = {
            name: hashlib.sha256((cwd / name).read_bytes()).hexdigest()
            for name in (
                "substitute/app/bootstrap/shared_splash_host.py",
                "substitute/presentation/shell/splash_window.py",
                "substitute/presentation/shell/splash_feedback.py",
                "substitute/presentation/shell/splash_progress_panel.py",
                "sugarsubstitute_shared/windows_process_family.py",
                "sugarsubstitute_shared/supervised_text_process.py",
                "launcher/sugarsubstitute_launcher/splash_session.py",
            )
        }
    receipt["launch_fixture_sha256"] = hashlib.sha256(Path(__file__).with_name("foreground_launcher.py").read_bytes()).hexdigest()
    fixture = ForegroundLauncher(native)
    try:
        fixture.__enter__()
        receipt["foreground_launch_setup"] = fixture.receipt
    except Exception as error:
        receipt["foreground_launch_setup"] = fixture.receipt
        receipt.update(error_type=type(error).__name__, error=str(error), failure_phase="before measured product launch")
        try:
            write_json(output / "failure.json", receipt)
        except Exception as storage:
            raise ExceptionGroup("Startup preparation and failure-receipt storage failed", [error, storage])
        raise
    process: subprocess.Popen[bytes] | None = None
    owner: ReferenceOwner | None = None
    primary_error: Exception | None = None
    try:
        started = time.perf_counter()
        process = subprocess.Popen(command, cwd=cwd, env=environment,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=0x08000000)
        receipt["pid"] = process.pid
        window_process = process
        deadline = started + arguments.duration_ms / 1000
        if arguments.mode == "reference":
            owner = ReferenceOwner(process, Path(command[0]), base, ReferenceProcesses())
            owner.bind(deadline)
            window_process = owner
            receipt.update(pid=owner.pid, launch_pid=process.pid, reference_process_owner=owner.metadata)
        fixture.allow_product(window_process.pid)
        hwnd = native.find_window(window_process, deadline)
        client = native.client(hwnd, window_process.pid)
        receipt.update(dpi=client.dpi, device_size=[client.width, client.height],
                       logical_size=[client.width / client.scale, client.height / client.scale])
        image, observed, exposed = observe_compositor(native, hwnd, window_process, deadline)
        fixture.product_exposed()
        visible_ms = (observed - started) * 1000
        receipt.update(first_owned_foreground_unoccluded_ms=(exposed - started) * 1000,
                       initial_exposure_status="owned foreground and unoccluded observed before compositor capture")
        before = rgb_pixel(image, (round(30 * client.scale), round(400 * client.scale)))
        # Both public surfaces begin collapsed. One real caption press/release expands output.
        receipt["caption_gesture"] = {"logical_point": [489, 16],
                                      "device_point": [round(489 * client.scale), round(16 * client.scale)],
                                      "input_source": "SendInput native mouse down/move/up"}
        with NativePointer(native, hwnd, window_process.pid) as pointer:
            pointer.move((489, 16))
            pointer.press()
            pointer.release()
            while True:
                changed = native.capture(hwnd, window_process.pid, compositor=True)
                after = rgb_pixel(changed, (round(30 * client.scale), round(400 * client.scale)))
                if max(abs(left - right) for left, right in zip(before, after)) > 5:
                    break
                if time.perf_counter() >= deadline or window_process.poll() is not None:
                    raise RuntimeError("The caption disclosure did not visibly respond to its first gesture")
                time.sleep(0.005)
            interactive_ms = (time.perf_counter() - started) * 1000
        receipt.update(startup=startup_decision(visible_ms, interactive_ms, profile=arguments.timing_profile),
                       observation_method="Owned unoccluded DWM compositor capture; first caption gesture response")
        image.save(output / "visible.png")
        changed.save(output / "interactive.png")
        native.post(hwnd, window_process.pid, 0x0010, 0, 0)
        stdout, stderr = process.communicate(timeout=5)
        # The production reference's ready message contains session authentication material.
        receipt.update(exit_code=process.returncode, stdout_bytes=len(stdout), stderr_bytes=len(stderr))
        if arguments.timing_audit:
            receipt["diagnostic_output"] = retain_diagnostics(arguments, output, stdout, stderr)
        if process.returncode != 0:
            raise RuntimeError(f"Owned startup fixture exited with status {process.returncode}")
        write_json(output / "startup.json", receipt)
        return receipt
    except Exception as error:
        primary_error = error
        if isinstance(error, ExposureFailure):
            receipt["exposure_state"] = error.state
        receipt.update(error_type=type(error).__name__, error=str(error))
        raise
    finally:
        cleanup_errors = []
        if owner is not None:
            try:
                owner.close()
            except Exception as error:
                cleanup_errors.append(error)
        try:
            if process is not None and process.poll() is None:
                process.terminate()
                process.communicate(timeout=5)
        except Exception as error:
            cleanup_errors.append(error)
        try:
            if arguments.timing_audit and process is not None and "diagnostic_output" not in receipt:
                stdout, stderr = process.communicate(timeout=5)
                receipt["diagnostic_output"] = retain_diagnostics(arguments, output, stdout, stderr)
        except Exception as error:
            cleanup_errors.append(error)
        try:
            fixture.__exit__(None, None, None)
        except Exception as error:
            cleanup_errors.append(error)
        if primary_error is not None or cleanup_errors:
            receipt["qualification_accepted"] = False
            if cleanup_errors:
                receipt["cleanup_errors"] = [{"error_type": type(error).__name__, "error": str(error)} for error in cleanup_errors]
            try:
                write_json(output / "failure.json", receipt)
            except Exception as storage:
                cleanup_errors.append(storage)
        if cleanup_errors:
            raise ExceptionGroup("Startup fixture and owned cleanup failures",
                                 ([primary_error] if primary_error is not None else []) + cleanup_errors)


def main() -> int:
    """Run sequential windows and keep first-in-series measurements separate from warm repeats."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True, choices=("rust", "reference"))
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--interpreter", type=Path)
    parser.add_argument("--legacy-root", type=Path)
    parser.add_argument("--temporary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--locale", default="en", choices=("en", "zh-Hans", "ja", "ko", "es"))
    parser.add_argument("--light", action="store_true")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--duration-ms", type=int, default=5000)
    parser.add_argument("--enforce-targets", action="store_true")
    parser.add_argument("--timing-audit", action="store_true", help="Collect bounded native Rust startup diagnostics")
    parser.add_argument("--timing-profile", choices=TIMING_PROFILES, default="strict")
    args = parser.parse_args()
    if not 1 <= args.runs <= 30 or not 1000 <= args.duration_ms <= 30000:
        parser.error("Use 1 through 30 sequential runs with a 1000 through 30000 ms lifetime")
    if args.mode == "rust" and args.executable is None:
        parser.error("Rust qualification requires --executable")
    if args.mode == "reference" and (args.interpreter is None or args.legacy_root is None):
        parser.error("Reference qualification requires --interpreter and --legacy-root")
    if args.timing_audit and args.mode != "rust":
        parser.error("--timing-audit requires --mode rust")
    args.output.mkdir(parents=True, exist_ok=False)
    native = NativeWindows()
    receipts = [observe(args, args.output / f"run-{ordinal:02}", ordinal, native) for ordinal in range(args.runs)]
    write_json(args.output / "series.json", receipts)
    return 1 if args.enforce_targets and any(not receipt["startup"]["accepted"] for receipt in receipts) else 0


if __name__ == "__main__":
    raise SystemExit(main())
