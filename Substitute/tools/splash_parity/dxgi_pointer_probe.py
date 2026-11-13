# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""One owned pointer-triggered format diagnostic; never pixels or performance proof.

Readiness precedes the native acquisition call, rather than synchronizing entry.
Raw texture evidence and pointer attribution are reported independently. Root
schedules this visible fixture; every native wait keeps the original deadline.
"""
from __future__ import annotations
import argparse
import ctypes
import hashlib
import json
import os
import re
import subprocess
import time
from ctypes import wintypes
from contextlib import contextmanager
from pathlib import Path
try:
    from .native_windows import NativeWindows, NativePointer, observe_exposure, write_json
    from .foreground_launcher import ForegroundLauncher
    from .startup_windows import ReferenceProcesses
    from .operational_windows import OperationalOutput, error_causes
    from .region_capture import RegionCapture
    from .region_records import Region, integer
    from .m0_windows import Qpc, sample_record
    from .native_timing import unique_object
    from .dxgi_known_color import FixtureControl, palette_bgra, color_equal
except ImportError:
    from native_windows import NativeWindows, NativePointer, observe_exposure, write_json
    from foreground_launcher import ForegroundLauncher
    from startup_windows import ReferenceProcesses
    from operational_windows import OperationalOutput, error_causes
    from region_capture import RegionCapture
    from region_records import Region, integer
    from m0_windows import Qpc, sample_record
    from native_timing import unique_object
    from dxgi_known_color import FixtureControl, palette_bgra, color_equal

LIMIT, POLLS = 16384, 1024
_THREAD_OWNERS = {}
_PROCESS_OWNERS = {}
READY_KEYS = {"schema_version", "event", "pid", "thread", "monitor", "qpc_frequency", "ready_qpc"}
FINAL_KEYS = {"schema_version", "acquisition_mode", "acquire_format_requested", "no_pixel_copy_or_map",
              "duplication_enrollments", "duplicate_output1_attempted", "original_duplicate_output_attempted",
              "acquisition_enrollment_api", "acquisition_requested_formats", "acquisition_enrollment_flags",
              "thread", "monitor", "output_index", "adapter_luid_low", "adapter_luid_high", "vendor_id", "device_id",
              "output_rect", "rotation", "color_space", "bits_per_color", "duplicate_output",
              "duplicate_output1_bgra_only", "duplicate_output1_hdr_inventory", "acquisition_attempts_requested",
              "first_before_owned_update_acquire", "format_only_acquire", "owned_desktop_update"}
ACQUISITION_KEYS = {"enroll_hresult", "attempted", "acquire_hresult", "query_hresult", "release_hresult",
                    "clock_hresult", "primary_hresult", "primary_operation", "secondary_hresult", "secondary_operation",
                    "frame_acquired", "release_attempts", "timeout_ms", "qpc_frequency", "acquire_begin_qpc",
                    "acquire_end_qpc", "last_present_qpc", "last_mouse_qpc", "accumulated_frames",
                    "protected_content_masked", "texture_format", "width", "height", "mip_levels", "array_size", "sample_count"}
INACTIVE_ACQUISITION = dict.fromkeys(ACQUISITION_KEYS, 0)
INACTIVE_ACQUISITION.update(attempted=False, frame_acquired=False, protected_content_masked=False,
                            acquire_hresult=1, query_hresult=1, release_hresult=1)
INACTIVE_UPDATE = dict(requested=False, create_hresult=1, paint_hresult=1, destroy_hresult=1, unregister_hresult=1,
                       paint_count=0, paint_qpc=0, window_retained=False, class_retained=False, visibility_hresult=1,
                       visible=False, topmost=False, owned_point_witnesses=False, window_rect=[0, 0, 0, 0],
                       client_rect=[0, 0, 0, 0], foreground_before=0, foreground_enrolled=0, foreground_after=0,
                       foreground_preserved=True, gdi_flush_hresult=1, dwm_flush_hresult=1, flush_clock_hresult=0,
                       gdi_begin_qpc=0, gdi_end_qpc=0, dwm_begin_qpc=0, dwm_end_qpc=0)


def enroll(path, digest):
    """Enroll bounded absolute executable bytes before any native side effect."""
    if not path.is_absolute() or not isinstance(digest, str) or re.fullmatch(r"[0-9a-fA-F]{64}", digest) is None:
        raise ValueError("Probe and fixture require absolute paths and exact SHA256")
    path = path.resolve(strict=True)
    if not path.is_file() or path.suffix.casefold() != ".exe" or path.stat().st_size > 32 * 1024 * 1024:
        raise ValueError("Diagnostic executable exceeds its enrollment contract")
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest.lower():
        raise ValueError("Diagnostic executable SHA256 differs")
    return path


def verify_process(processes, process, executable, creation=None, *, live=True):
    """Verify live paths; terminal observations retain the previously enrolled PID/creation."""
    handle = int(process._handle)
    if creation is not None:
        integer(creation, 1, 0xFFFFFFFFFFFFFFFF, "verified process creation")
    if live:
        pid, path, observed = processes.identity(handle)
        if path != executable:
            raise RuntimeError("Retained diagnostic process path/creation/liveness differs")
    else:
        if creation is None:
            raise ValueError("Terminal process identity requires previously verified creation")
        pid, observed = processes.lifetime_identity(handle)
    integer(pid, 1, 0xFFFFFFFF, "retained process PID")
    integer(observed, 1, 0xFFFFFFFFFFFFFFFF, "retained process creation")
    if (pid != process.pid
            or creation is not None and observed != creation
            or live and (not processes.alive(handle) or process.poll() is not None)):
        raise RuntimeError("Retained diagnostic process path/creation/liveness differs")
    return observed


class ProbePipe:
    """Own only nonblocking pipe availability and a retained native readiness thread.

    SDK26100 declares these query APIs. The process handle remains Popen-owned;
    a thread handle is retained until CloseHandle actually succeeds.
    """
    def __init__(self, kernel):
        self.kernel, self.thread_handle = kernel, None
        h, d = wintypes.HANDLE, wintypes.DWORD
        for name, args, result in (
            ("PeekNamedPipe", [h, ctypes.c_void_p, d, ctypes.c_void_p, ctypes.POINTER(d), ctypes.c_void_p], wintypes.BOOL),
            ("OpenThread", [d, wintypes.BOOL, d], h), ("GetProcessIdOfThread", [h], d),
            ("GetThreadTimes", [h] + [ctypes.POINTER(wintypes.FILETIME)] * 4, wintypes.BOOL)):
            function = getattr(kernel, name)
            function.argtypes, function.restype = args, result

    def available(self, stream):
        import msvcrt
        count = wintypes.DWORD()
        if not self.kernel.PeekNamedPipe(msvcrt.get_osfhandle(stream.fileno()), None, 0, None, ctypes.byref(count), None):
            raise ctypes.WinError(ctypes.get_last_error())
        return count.value

    def verify_thread(self, thread, pid, creation):
        if self.thread_handle or len(_THREAD_OWNERS) >= 8:
            raise RuntimeError("Outstanding diagnostic thread ownership exceeds its bounded inventory")
        _THREAD_OWNERS[id(self)] = self
        self.thread_handle = self.kernel.OpenThread(0x0800, False, thread)
        if not self.thread_handle:
            _THREAD_OWNERS.pop(id(self))
            raise ctypes.WinError(ctypes.get_last_error())
        stamps = [wintypes.FILETIME() for _ in range(4)]
        actual = self.kernel.GetProcessIdOfThread(self.thread_handle)
        if not actual:
            raise ctypes.WinError(ctypes.get_last_error())
        if actual != pid:
            raise RuntimeError("Readiness thread does not belong to the retained probe")
        if not self.kernel.GetThreadTimes(self.thread_handle, *(ctypes.byref(s) for s in stamps)):
            raise ctypes.WinError(ctypes.get_last_error())
        born = stamps[0].dwHighDateTime << 32 | stamps[0].dwLowDateTime
        if born < creation or self.kernel.GetProcessIdOfThread(self.thread_handle) != pid:
            raise RuntimeError("Readiness thread predates or changed its probe identity")
        return born

    def close(self):
        if self.thread_handle:
            if not self.kernel.CloseHandle(self.thread_handle):
                raise ctypes.WinError(ctypes.get_last_error())
            self.thread_handle = None
        _THREAD_OWNERS.pop(id(self), None)


def decode_records(raw):
    if type(raw) is not bytes or len(raw) > LIMIT:
        raise ValueError("Diagnostic stdout exceeds its fixed payload limit")
    return [json.loads(line, object_pairs_hook=unique_object) for line in raw.decode("ascii", "strict").splitlines()]


def validate_ready(record, pid, monitor, frequency, earliest, observed):
    if not isinstance(record, dict) or set(record) != READY_KEYS:
        raise ValueError("Unknown acquisition readiness schema")
    for key, expected in (("schema_version", 1), ("pid", pid), ("monitor", monitor), ("qpc_frequency", frequency)):
        if type(record[key]) is not int or record[key] != expected:
            raise ValueError("Acquisition readiness identity/monitor/clock differs")
    integer(record["thread"], 1, 0xFFFFFFFF, "probe thread")
    integer(record["ready_qpc"], earliest, observed, "readiness QPC")
    if record["event"] != "format_acquisition_ready":
        raise ValueError("Unknown acquisition readiness event")
    return record


def wait_ready(pipe, processes, process, executable, creation, monitor, clock, earliest, deadline, state):
    """Read available bytes only, with one original deadline and fixed inventory."""
    raw = bytearray()
    state["readiness"] = dict(polls=0, prefix_hex="")
    for attempt in range(POLLS):
        if time.perf_counter() >= deadline:
            raise RuntimeError("Probe readiness exceeded the original launch deadline")
        verify_process(processes, process, executable, creation)
        count = pipe.available(process.stdout)
        state["readiness"]["polls"] = attempt + 1
        if count:
            integer(count, 1, LIMIT - len(raw), "available readiness bytes")
            part = process.stdout.read(count)
            if type(part) is not bytes or len(part) != count:
                raise RuntimeError("Available probe pipe bytes were not completely observed")
            raw.extend(part)
            state["readiness"]["prefix_hex"] = raw.hex()
            if b"\n" in raw:
                line = bytes(raw).split(b"\n", 1)[0]
                if len(line) > 1024:
                    raise ValueError("Readiness line exceeds its fixed payload limit")
                observed = clock.read()
                state["readiness"]["observed_qpc"] = observed
                records = decode_records(line)
                if len(records) != 1:
                    raise ValueError("Probe readiness contains extra records")
                record = validate_ready(records[0], process.pid, monitor, clock.frequency, earliest, observed)
                state["readiness"]["record"] = record
                state["readiness"]["thread_creation_ticks"] = pipe.verify_thread(record["thread"], process.pid, creation)
                verify_process(processes, process, executable, creation)
                if time.perf_counter() >= deadline:
                    raise RuntimeError("Validated readiness exceeded the original launch deadline")
                return bytes(raw), record
        remaining = deadline - time.perf_counter()
        if remaining > 0:
            time.sleep(min(.005, remaining))
    raise RuntimeError("Probe readiness exceeded its fixed polling inventory")


def initial_palette(capture, deadline, state):
    """Await two consecutive exact RGB observations within the original launch bound."""
    state["initial_gdi"] = []
    progress = state["initial_palette_readiness"] = dict(attempts=0, limit=256, consecutive=0, completed=False)
    previous_geometry = None
    for _ in range(256):
        if time.perf_counter() >= deadline:
            raise RuntimeError("Initial GDI palette exceeded the original launch deadline")
        progress["attempts"] += 1
        sample = capture.sample()
        state["initial_gdi"].append(sample_record(sample))
        if time.perf_counter() >= deadline:
            raise RuntimeError("Initial GDI palette exceeded the original launch deadline")
        if color_equal(sample.bgra, palette_bgra(0)):
            progress["consecutive"] = progress["consecutive"] + 1 if sample.geometry == previous_geometry else 1
            previous_geometry = sample.geometry
        else:
            progress["consecutive"], previous_geometry = 0, None
        if progress["consecutive"] == 2:
            progress["completed"] = True
            return sample.geometry
    raise RuntimeError("Initial GDI palette exhausted its 256 observation inventory")


def movement(pointer, point, clock, deadline, state):
    """Keep the common pointer owner, with full-cost QPC and actual-placement evidence."""
    receipt = dict(target=list(point), requested_events=1, event_flags=0xE001,
                   mode="NativePointer.move_screen SendInput", completed=False,
                   insertion_result="Not exposed by the common pointer owner")
    state.append(receipt)
    receipt["begin_qpc"] = clock.read()
    primary, causes = None, []
    try:
        if time.perf_counter() >= deadline:
            raise RuntimeError("Pointer movement exceeded the original launch deadline")
        if any(pointer.native.user.GetAsyncKeyState(key) & 0x8000 for key in (1, 2, 4, 5, 6)):
            pointer.original = None
            raise RuntimeError("Human mouse button state prevents diagnostic movement")
        pointer.move_screen(point)
        receipt["completed"] = True
        receipt["actual_cursor"] = list(pointer.last)
        if max(abs(a - b) for a, b in zip(pointer.last, point)) > 1:
            raise RuntimeError("Observed diagnostic cursor placement differs")
    except BaseException as error:
        primary = error
    finally:
        receipt["owner_last_position"] = list(pointer.last) if pointer.last is not None else None
        try:
            receipt["actual_cursor"] = list(pointer.cursor())
        except BaseException as error:
            causes.append(error)
        try:
            if any(pointer.native.user.GetAsyncKeyState(key) & 0x8000 for key in (1, 2, 4, 5, 6)):
                pointer.original = None
                raise RuntimeError("Human mouse button state appeared during diagnostic movement")
        except BaseException as error:
            causes.append(error)
        try:
            receipt["end_qpc"] = clock.read()
        except BaseException as error:
            causes.append(error)
    if primary or causes:
        errors = ([primary] if primary else []) + causes
        receipt["failure_causes"] = [cause for error in errors for cause in error_causes(error)]
        raise BaseExceptionGroup("Diagnostic movement and observation failed", errors)
    if receipt["end_qpc"] < receipt["begin_qpc"] or time.perf_counter() >= deadline:
        raise RuntimeError("Diagnostic movement clock/deadline differs")
    return receipt


@contextmanager
def pointer_lifetime(native, hwnd, pid, state):
    """Guard human button arrival through terminal collection before shared cleanup."""
    with NativePointer(native, hwnd, pid) as pointer:
        primary = None
        try:
            yield pointer
        except BaseException as error:
            primary = error
            raise
        finally:
            try:
                if any(native.user.GetAsyncKeyState(key) & 0x8000 for key in (1, 2, 4, 5, 6)):
                    pointer.original = None
                    state["cursor_restoration_suppressed"] = "Human mouse button state at diagnostic teardown"
                    raise RuntimeError("Human button state prevents diagnostic cursor restoration")
            except BaseException as error:
                pointer.original = None
                if primary:
                    raise BaseExceptionGroup("Pointer diagnostic and human-state guard failed", [primary, error])
                raise


def wait_terminal(processes, process, path, creation, deadline, state):
    """Observe actual exit without resetting the original deadline before draining."""
    state["terminal_observation"] = dict(polls=0, exit_code=None)
    for attempt in range(POLLS):
        if time.perf_counter() >= deadline:
            raise RuntimeError("Probe exit exceeded the original launch deadline")
        verify_process(processes, process, path, creation, live=False)
        code = process.poll()
        state["terminal_observation"].update(polls=attempt + 1, exit_code=code)
        if code is not None:
            return
        remaining = deadline - time.perf_counter()
        if remaining > 0:
            time.sleep(min(.005, remaining))
    raise RuntimeError("Probe exit exceeded its fixed observation inventory")


def retain_native_records(records, state):
    """Retain query/release effects before a later controller clock can fail."""
    state["native_records"] = records
    if len(records) != 2 or not isinstance(records[1], dict):
        return
    acquisition = records[1].get("format_only_acquire")
    fields = ("texture_format", "width", "height", "mip_levels", "array_size", "sample_count")
    acquired = (isinstance(acquisition, dict) and acquisition.get("frame_acquired") is True
        and all(type(acquisition.get(key)) is int and acquisition[key] == value
                for key, value in (("acquire_hresult", 0), ("query_hresult", 0), ("release_hresult", 0), ("release_attempts", 1)))
        and all(type(acquisition.get(key)) is int and 1 <= acquisition[key] <= 0xFFFFFFFF for key in fields))
    state["actual_texture"] = {key: acquisition[key] for key in fields} if acquired else None
    state["actual_format_status"] = "acquired and queried and released" if acquired else "unknown"


def inactive_record(record, expected):
    """Require the native default POD, proving that this mode performed no work there."""
    if not isinstance(record, dict) or set(record) != set(expected):
        raise ValueError("Inactive native diagnostic record schema differs")
    for key, value in expected.items():
        actual = record[key]
        if type(actual) is not type(value) or actual != value:
            raise ValueError("Inactive native diagnostic contains an unexpected effect or scalar")
        if type(value) is list and any(type(item) is not int for item in actual):
            raise ValueError("Inactive native rectangle requires exact integer coordinates")


def inactive_update(record):
    """Validate zero update effects while retaining its actual final foreground observation."""
    if not isinstance(record, dict) or set(record) != set(INACTIVE_UPDATE):
        raise ValueError("Inactive native update schema differs")
    after = integer(record["foreground_after"], 0, (1 << (ctypes.sizeof(ctypes.c_void_p) * 8)) - 1,
                    "observed foreground HWND")
    expected = dict(INACTIVE_UPDATE, foreground_after=after, foreground_preserved=after == INACTIVE_UPDATE["foreground_before"])
    inactive_record(record, expected)


def final_evidence(records, ready, move, observed, state):
    """Separate texture description evidence from actual pointer-update attribution."""
    retain_native_records(records, state)
    if len(records) != 2 or records[0] != ready or not isinstance(records[1], dict) or set(records[1]) != FINAL_KEYS:
        raise ValueError("Expected exactly readiness then original acquisition metadata")
    final = records[1]
    expected = dict(schema_version=1, acquisition_mode="pointer_trigger_once", acquire_format_requested=True,
                    no_pixel_copy_or_map=True, duplication_enrollments=1, duplicate_output1_attempted=False,
                    original_duplicate_output_attempted=True, acquisition_enrollment_api="DuplicateOutput",
                    acquisition_requested_formats=[], acquisition_enrollment_flags=0, acquisition_attempts_requested=1,
                    thread=ready["thread"], monitor=ready["monitor"])
    if any(type(final[key]) is not type(value) or final[key] != value for key, value in expected.items()):
        raise ValueError("Final probe enrollment/identity contract differs")
    for key in ("output_index", "adapter_luid_low", "vendor_id", "device_id", "rotation", "color_space", "bits_per_color"):
        integer(final[key], 0, 0xFFFFFFFF, key)
    integer(final["adapter_luid_high"], -(1 << 31), (1 << 31) - 1, "adapter LUID high")
    rect = final["output_rect"]
    if type(rect) is not list or len(rect) != 4 or any(type(value) is not int or not -(1 << 31) <= value < (1 << 31) for value in rect):
        raise ValueError("Native output rectangle schema differs")
    if rect[2] <= rect[0] or rect[3] <= rect[1]:
        raise ValueError("Native output has invalid physical bounds")
    descriptor_keys = {"hresult", "format", "width", "height", "rotation"}
    for key in ("duplicate_output", "duplicate_output1_bgra_only", "duplicate_output1_hdr_inventory"):
        descriptor = final[key]
        if not isinstance(descriptor, dict) or set(descriptor) != descriptor_keys | ({"attempted"} if key.endswith("hdr_inventory") else set()):
            raise ValueError("Native duplication descriptor schema differs")
        integer(descriptor["hresult"], -(1 << 31), (1 << 31) - 1, "descriptor HRESULT")
        for field in descriptor_keys - {"hresult"}:
            integer(descriptor[field], 0, 0xFFFFFFFF, field)
        if key.endswith("hdr_inventory") and descriptor["attempted"] is not False:
            raise ValueError("Pointer diagnostic performed a modern-format inventory")
    inactive_record(final["duplicate_output1_bgra_only"], dict(hresult=1, format=0, width=0, height=0, rotation=0))
    inactive_record(final["duplicate_output1_hdr_inventory"], dict(attempted=False, hresult=1, format=0, width=0, height=0, rotation=0))
    if final["duplicate_output"]["hresult"] != 0:
        raise ValueError("Original duplication descriptor disagrees with the enrolled acquisition")
    acquisition = final["format_only_acquire"]
    if not isinstance(acquisition, dict) or set(acquisition) != ACQUISITION_KEYS:
        raise ValueError("Missing original format acquisition evidence")
    for key in ("enroll_hresult", "acquire_hresult", "query_hresult", "release_hresult", "clock_hresult",
                "primary_hresult", "secondary_hresult"):
        integer(acquisition[key], -(1 << 31), (1 << 31) - 1, key)
    for key in ("primary_operation", "secondary_operation", "release_attempts", "timeout_ms", "accumulated_frames",
                "texture_format", "width", "height", "mip_levels", "array_size", "sample_count"):
        integer(acquisition[key], 0, 0xFFFFFFFF, key)
    for key in ("attempted", "frame_acquired", "protected_content_masked"):
        if type(acquisition[key]) is not bool:
            raise ValueError("Native acquisition boolean differs")
    acquired = (acquisition["frame_acquired"] and acquisition["acquire_hresult"] == 0
                and acquisition["query_hresult"] == acquisition["release_hresult"] == 0
                and acquisition["release_attempts"] == 1)
    if acquired and any(acquisition[key] == 0 for key in ("texture_format", "width", "height", "mip_levels", "array_size", "sample_count")):
        raise ValueError("Acquired texture has invalid description dimensions")
    integer(acquisition["qpc_frequency"], 1, 0x7FFFFFFFFFFFFFFF, "native QPC frequency")
    integer(acquisition["last_present_qpc"], 0, observed, "last present QPC")
    integer(acquisition["last_mouse_qpc"], 0, observed, "last mouse QPC")
    begin = integer(acquisition["acquire_begin_qpc"], ready["ready_qpc"], observed, "acquire begin")
    end = integer(acquisition["acquire_end_qpc"], begin, observed, "acquire end")
    inactive_record(final["first_before_owned_update_acquire"], INACTIVE_ACQUISITION)
    inactive_update(final["owned_desktop_update"])
    measured = move.get("completed") is True and "end_qpc" in move and not move.get("failure_causes")
    state["pointer_trigger_attributed"] = (acquired and measured
        and ready["ready_qpc"] <= move["begin_qpc"] <= end
        and move["begin_qpc"] <= acquisition["last_mouse_qpc"] <= end and acquisition["last_mouse_qpc"] > 0)
    state["pointer_only"] = acquired and acquisition["last_present_qpc"] == 0
    state["movement_relation"] = ("unverified_movement" if not measured else
                                  "before_native_acquire" if move["end_qpc"] < begin else
                                  "after_native_acquire" if move["begin_qpc"] > end else "overlaps_native_acquire")
    if (not acquisition["attempted"] or acquisition["enroll_hresult"] != 0 or acquisition["timeout_ms"] != 500
            or acquisition["qpc_frequency"] != ready["qpc_frequency"] or acquisition["clock_hresult"] != 0):
        raise ValueError("Native single acquisition clock/enrollment differs")
    timeout = (acquisition["acquire_hresult"] == -2005270489 and not acquisition["frame_acquired"]
               and acquisition["query_hresult"] == acquisition["release_hresult"] == 1 and acquisition["release_attempts"] == 0)
    if not acquired and not timeout:
        raise RuntimeError("Native format acquisition did not settle successfully or time out")


def run(args):
    fixture, probe = enroll(args.fixture, args.fixture_sha256), enroll(args.probe, args.probe_sha256)
    if os.name == "nt" and args.output.resolve().drive.casefold() != "d:":
        raise ValueError("Native diagnostic artifacts must use D:")
    args.output.mkdir(parents=True, exist_ok=False)
    temporary = args.output / "temporary"
    temporary.mkdir()
    environment = dict(os.environ, TEMP=str(temporary), TMP=str(temporary), PYTHONDONTWRITEBYTECODE="1")
    native, processes, clock = NativeWindows(), ReferenceProcesses(), Qpc()
    launcher, pipe = ForegroundLauncher(native), ProbePipe(processes.kernel)
    state = dict(schema_version=1, experiment_completed=False, pointer_trigger_attributed=False,
                 pixel_accepted=False, performance_accepted=False, movements=[], processes={},
                 binaries={"fixture": dict(path=str(fixture), sha256=args.fixture_sha256.lower()),
                           "probe": dict(path=str(probe), sha256=args.probe_sha256.lower())})
    sources = ("dxgi_pointer_probe.py", "native_windows.py", "foreground_launcher.py", "startup_windows.py",
               "operational_windows.py", "region_capture.py", "region_records.py", "gdi_storage.py", "m0_windows.py",
               "native_timing.py", "dxgi_known_color.py")
    state["harness_sha256"] = {name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest() for name in sources}
    owned, hwnd, primary, cleanup, prefix = [], None, None, [], b""
    try:
        launcher.__enter__()
        deadline = time.perf_counter() + 5
        state["launch_deadline_seconds"] = deadline
        def launch(name, path, arguments):
            if len(_PROCESS_OWNERS) >= 8:
                raise RuntimeError("Unsettled diagnostic processes exhausted their bounded inventory")
            target = args.output / name
            target.mkdir()
            process = subprocess.Popen([str(path), *arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                       env=environment, creationflags=0x08000000)
            output = OperationalOutput(process, target)
            _PROCESS_OWNERS[id(process)] = (process, output, path)
            owned.append((name, process, output, path))
            state["processes"][name] = dict(pid=process.pid)
            creation = verify_process(processes, process, path)
            state["processes"][name]["creation_ticks"] = creation
            return process, output, creation
        fixture_process, fixture_output, creation = launch("fixture", fixture, [])
        hwnd = native.find_window(fixture_process, deadline)
        state["hwnd"] = hwnd
        pid = ctypes.c_uint32()
        thread = native.user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        state["fixture_window_identity"] = dict(pid=pid.value, thread=thread)
        integer(thread, 1, 0xFFFFFFFF, "fixture thread")
        if pid.value != fixture_process.pid or not native.owns(hwnd, fixture_process.pid) or not native.user.IsWindowVisible(hwnd):
            raise RuntimeError("Fixture window ownership differs")
        verify_process(processes, fixture_process, fixture, creation)
        if time.perf_counter() >= deadline:
            raise RuntimeError("Fixture admission exceeded original launch deadline")
        state["foreground_permission"] = dict(pid=fixture_process.pid, attempted=True, completed=False)
        launcher.allow_product(fixture_process.pid)
        state["foreground_permission"]["completed"] = True
        state["exposure_seconds"] = observe_exposure(native, hwnd, fixture_process, deadline)
        launcher.product_exposed()
        with RegionCapture(hwnd, fixture_process.pid, Region(40, 40, 2, 2), creation_ticks=creation) as capture:
            geometry = initial_palette(capture, deadline, state)
            monitor = native.user.MonitorFromPoint(wintypes.POINT(geometry.screen_x + 40, geometry.screen_y + 40), 0)
            integer(monitor, 1, (1 << 64) - 1, "actual ROI monitor")
            state["monitor"] = monitor
            center = FixtureControl(native).point(hwnd, fixture_process.pid, thread)
            points = ((center[0] - 16, center[1]), (center[0] + 16, center[1]))
            if any(abs(x - (geometry.screen_x + 40)) <= 32 and abs(y - (geometry.screen_y + 40)) <= 32 for x, y in points):
                raise RuntimeError("Diagnostic pointer points are not clear of the pixel ROI")
            with pointer_lifetime(native, hwnd, fixture_process.pid, state) as pointer:
                movement(pointer, points[0], clock, deadline, state["movements"])
                earliest = clock.read()
                probe_process, probe_output, probe_creation = launch("probe", probe, ["--acquire-format-pointer-trigger-once"])
                prefix, ready = wait_ready(pipe, processes, probe_process, probe, probe_creation,
                                           monitor, clock, earliest, deadline, state)
                move = movement(pointer, points[1], clock, deadline, state["movements"])
                wait_terminal(processes, probe_process, probe, probe_creation, deadline, state)
                probe_output.collect("single_format_probe_terminal")
                raw = prefix + probe_output.stdout
                state["probe_stdout_bytes"] = len(raw)
                (args.output / "probe-stdout.bin").write_bytes(raw[:LIMIT])
                records = decode_records(raw)
                retain_native_records(records, state)
                final_evidence(records, ready, move, clock.read(), state)
                if time.perf_counter() >= deadline:
                    raise RuntimeError("Probe terminal metadata exceeded the original launch deadline")
                if probe_process.returncode != 0 or probe_output.stderr:
                    raise RuntimeError("Owned native probe did not terminate successfully")
                verify_process(processes, probe_process, probe, probe_creation, live=False)
        state["experiment_completed"] = True
    except BaseException as error:
        primary = error
        if hasattr(error, "state"):
            state["native_failure_state"] = error.state
        raise
    finally:
        for name, process, output, path in reversed(owned):
            try:
                state["processes"][name]["before_cleanup"] = dict(exit_code=process.poll(), output_complete=output.complete)
                if not output.complete and name == "fixture" and hwnd is not None and process.poll() is None and native.owns(hwnd, process.pid):
                    native.post(hwnd, process.pid, 0x0010, 0, 0)
                    output.collect("fixture_owned_graceful_close")
            except BaseException as error:
                cleanup.append(error)
            if not output.complete:
                try:
                    output.failed_teardown()
                except BaseException as error:
                    cleanup.append(error)
            state["processes"][name]["output"] = output.receipt()
            if output.complete:
                _PROCESS_OWNERS.pop(id(process), None)
            else:
                state["processes"][name]["recovery_owner_retained"] = True
            if name == "probe":
                try:
                    raw = bytes.fromhex(state.get("readiness", {}).get("prefix_hex", "")) + output.stdout
                    state["probe_stdout_bytes"] = len(raw)
                    (args.output / "probe-stdout.bin").write_bytes(raw[:LIMIT])
                    records = decode_records(raw)
                    retain_native_records(records, state)
                    if primary and state.get("readiness", {}).get("record") and len(state["movements"]) == 2:
                        final_evidence(records, state["readiness"]["record"], state["movements"][-1], clock.read(), state)
                except BaseException as error:
                    cleanup.append(error)
            try:
                enroll(path, state["binaries"][name]["sha256"])
            except BaseException as error:
                cleanup.append(error)
        for close in (pipe.close, lambda: launcher.__exit__(None, None, None)):
            try:
                close()
            except BaseException as error:
                cleanup.append(error)
        state["foreground_launch_setup"] = launcher.receipt
        if primary or cleanup:
            state["experiment_completed"] = state["pointer_trigger_attributed"] = False
            state["failure_causes"] = [cause for error in ([primary] if primary else []) + cleanup for cause in error_causes(error)]
        try:
            write_json(args.output / "observation.json", state)
        except BaseException as error:
            cleanup.append(error)
        if cleanup:
            raise BaseExceptionGroup("Pointer diagnostic and owned cleanup failed", ([primary] if primary else []) + cleanup)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("fixture", "probe"):
        parser.add_argument(f"--{name}", type=Path, required=True)
        parser.add_argument(f"--{name}-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
