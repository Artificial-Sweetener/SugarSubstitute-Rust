# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Qualify the exact DXGI helper using an owned opaque native color fixture.

This is device/pixel/lifecycle qualification, not a product performance campaign.
Root schedules the visible fixture. GDI corroboration is outside the measured
click-to-DXGI interval; neither observer's cost is subtracted.
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
from pathlib import Path
try:
    from .dxgi_abi import enroll_helper, DxgiLibrary
    from .dxgi_storage import DxgiCopyStorage
    from .region_capture import RegionCapture
    from .region_records import Region, RegionSample, integer
    from .m0_windows import Qpc, capture_attempt, sample_record
    from .native_windows import NativeWindows, NativePointer, observe_exposure, write_json
    from .foreground_launcher import ForegroundLauncher
    from .operational_windows import OperationalOutput, error_causes
    from .startup_windows import ReferenceProcesses
    from .native_timing import unique_object
except ImportError:
    from dxgi_abi import enroll_helper, DxgiLibrary
    from dxgi_storage import DxgiCopyStorage
    from region_capture import RegionCapture
    from region_records import Region, RegionSample, integer
    from m0_windows import Qpc, capture_attempt, sample_record
    from native_windows import NativeWindows, NativePointer, observe_exposure, write_json
    from foreground_launcher import ForegroundLauncher
    from operational_windows import OperationalOutput, error_causes
    from startup_windows import ReferenceProcesses
    from native_timing import unique_object

PALETTES = (((31, 37, 43), (43, 211, 59), (71, 83, 227), (191, 137, 101)),
            ((229, 233, 237), (173, 47, 113), (89, 229, 151), (251, 193, 67)))


def palette_bgra(index):
    return bytes(value for rgb in PALETTES[index] for value in (*reversed(rgb), 255))


def color_equal(actual, expected):
    return type(actual) is bytes and len(actual) == len(expected) == 16 and all(
        a == b for index, (a, b) in enumerate(zip(actual, expected)) if index % 4 != 3)


def observe_palette(capture, clock, index, minimum, deadline, inventory):
    """One fixed256 inventory/absolute deadline; pending never counts as pixels."""
    for _ in range(256):
        sample = capture_attempt(capture, clock, deadline, minimum, inventory)
        if type(sample) is RegionSample and color_equal(sample.bgra, palette_bgra(index)):
            return sample
    raise RuntimeError("Known palette exceeded bounded observation inventory")


def verify_paints(raw, pid, hwnd, thread, frequency, first, second, input_begin):
    """Corroborate fixture-authored palette and QPC metadata after actual owned exit."""
    if type(raw) is not bytes or len(raw) > 4096:
        raise RuntimeError("Known-color fixture output exceeds its bounded contract")
    records = [json.loads(line, object_pairs_hook=unique_object) for line in raw.decode("ascii", "strict").splitlines()]
    if len(records) != 2:
        raise RuntimeError("Known-color fixture did not emit exactlytwo painted revisions")
    keys = {"event", "pid", "hwnd", "thread", "revision", "paint_qpc", "qpc_frequency", "roi", "button_id", "bgra"}
    for index, (record, sample) in enumerate(zip(records, (first, second))):
        if not isinstance(record, dict) or set(record) != keys:
            raise RuntimeError("Unknown known-color paint record schema")
        for name, expected in (("pid", pid), ("hwnd", hwnd), ("thread", thread), ("revision", index + 1),
                               ("qpc_frequency", frequency), ("button_id", 1001)):
            if type(record[name]) is not int or record[name] != expected:
                raise RuntimeError("Known-color fixture identity/revision differs")
        if record["event"] != "painted":
            raise RuntimeError("Unknown known-color event")
        tick = integer(record["paint_qpc"], 1, sample.end_ticks, "paint QPC")
        if index == 1 and tick < input_begin:
            raise RuntimeError("Changed native paint predates the actual input")
        expected_roi = dict(x=sample.geometry.screen_x + 40, y=sample.geometry.screen_y + 40, width=2, height=2)
        if record["roi"] != expected_roi or any(type(value) is not int for value in record["roi"].values()):
            raise RuntimeError("Known-color painted screen ROI differs")
        expected = [list(palette_bgra(index)[offset:offset + 4]) for offset in range(0, 16, 4)]
        if record["bgra"] != expected or any(type(value) is not int for pixel in record["bgra"] for value in pixel):
            raise RuntimeError("Known-color authored palette differs")
    if records[1]["paint_qpc"] <= records[0]["paint_qpc"]:
        raise RuntimeError("Known-color revision timing is reversed")
    return records


class FixtureControl:
    """Verify only the fixture-specific standard button contract, not capture rules."""
    def __init__(self, native):
        self.native, self.user = native, native.user
        h = ctypes.c_void_p
        for name, args, result in (("GetDlgItem", [h, ctypes.c_int], h),
                                  ("GetClassNameW", [h, ctypes.c_wchar_p, ctypes.c_int], ctypes.c_int),
                                  ("GetParent", [h], h)):
            call = getattr(self.user, name)
            call.argtypes, call.restype = args, result

    def point(self, hwnd, pid, thread):
        if not self.native.owns(hwnd, pid):
            raise RuntimeError("Fixture root identity changed")
        text = ctypes.create_unicode_buffer(128)
        if self.user.GetClassNameW(hwnd, text, 128) != len("SS.DxgiColorFixture.V1") or text.value != "SS.DxgiColorFixture.V1":
            raise RuntimeError("The exact native color fixture class was not observed")
        button = self.user.GetDlgItem(hwnd, 1001)
        if not button or self.user.GetParent(button) != hwnd or not self.native.owns(button, pid):
            raise RuntimeError("Known-color change button is not an owned direct child")
        identity = ctypes.c_uint32()
        if self.user.GetWindowThreadProcessId(button, ctypes.byref(identity)) != thread or identity.value != pid:
            raise RuntimeError("Known-color change button thread changed")
        if self.user.GetClassNameW(button, text, 128) != 6 or text.value.casefold() != "button":
            raise RuntimeError("Known-color change child is not a native BUTTON")
        if not self.user.IsWindowVisible(button) or not self.user.IsWindowEnabled(button):
            raise RuntimeError("Known-color change button is not actionable")
        rect = wintypes.RECT()
        if not self.user.GetWindowRect(button, ctypes.byref(rect)) or rect.right <= rect.left or rect.bottom <= rect.top:
            raise RuntimeError("Known-color button bounds are invalid")
        return ((rect.left + rect.right) // 2, (rect.top + rect.bottom) // 2)


def run(args):
    helper = enroll_helper(args.capture_helper, args.capture_helper_sha256)
    if helper is None:
        raise ValueError("Known-color qualification requires the explicit DXGI helper pair")
    if not isinstance(args.fixture_sha256, str) or re.fullmatch(r"[0-9a-fA-F]{64}", args.fixture_sha256) is None:
        raise ValueError("Known-color fixture SHA256 must be exactly64 hexadecimal characters")
    executable = args.fixture.resolve(strict=True)
    digest = hashlib.sha256(executable.read_bytes()).hexdigest()
    if digest != args.fixture_sha256.lower():
        raise ValueError("Known-color fixture SHA256 differs")
    if os.name == "nt" and args.output.resolve().drive.casefold() != "d:":
        raise ValueError("Native qualification artifacts must use D:")
    # Enroll both payload identities before loading the DLL or creating any window.
    library = DxgiLibrary(helper)
    args.output.mkdir(parents=True, exist_ok=False)
    temporary = args.output / "temporary"
    temporary.mkdir()
    environment = os.environ.copy()
    environment.update(TEMP=str(temporary), TMP=str(temporary), PYTHONDONTWRITEBYTECODE="1")
    native, processes, clock = NativeWindows(), ReferenceProcesses(), Qpc()
    launcher = ForegroundLauncher(native)
    sources = ("dxgi_known_color.py", "dxgi_abi.py", "dxgi_storage.py", "region_capture.py", "region_records.py",
               "gdi_storage.py", "m0_windows.py", "native_windows.py", "native_timing.py", "foreground_launcher.py", "operational_windows.py", "startup_windows.py")
    state = dict(schema_version=1, capture_helper=dict(path=str(helper.path), sha256=helper.sha256),
                 harness_sha256={name: hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest() for name in sources},
                 fixture_sha256=digest, accepted=False,
                 corroboration="Two stable exact four-pixel RGB byte reads per palette; GDI alpha is unspecified",
                 scope="Actual opaque native color and copy-lifetime qualification; no product performance claim")
    primary, cleanup, hwnd, process, output, capture_owner = None, [], None, None, None, None
    try:
        launcher.__enter__()
        state["foreground_launch_setup"] = launcher.receipt
        launched = time.perf_counter()
        deadline = launched + 5
        state["launch_clock_seconds"], state["launch_deadline_seconds"] = launched, deadline
        process = subprocess.Popen([str(executable)], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   env=environment, creationflags=0x08000000)
        state["pid"] = process.pid
        output = OperationalOutput(process, args.output)
        pid, path, creation = processes.identity(int(process._handle))
        if (pid, path) != (process.pid, executable) or not processes.alive(int(process._handle)):
            raise RuntimeError("Owned fixture process identity differs")
        state["creation_ticks"] = creation
        hwnd = native.find_window(process, deadline)
        state["hwnd"] = hwnd
        if time.perf_counter() >= deadline:
            raise RuntimeError("Owned fixture window was observed after the original launch deadline")
        identity = ctypes.c_uint32()
        thread = native.user.GetWindowThreadProcessId(hwnd, ctypes.byref(identity))
        state["window_identity"] = dict(pid=identity.value, ui_thread_id=thread)
        if (identity.value != pid or type(thread) is not int or not 1 <= thread <= 0xFFFFFFFF
                or not native.owns(hwnd, pid) or not native.user.IsWindowVisible(hwnd)):
            raise RuntimeError("Owned fixture window thread differs")
        if (processes.identity(int(process._handle)) != (pid, executable, creation)
                or not processes.alive(int(process._handle)) or process.poll() is not None):
            raise RuntimeError("Owned fixture process changed before foreground permission")
        state.update(hwnd=hwnd, ui_thread_id=thread, qpc_frequency=clock.frequency)
        if time.perf_counter() >= deadline:
            raise RuntimeError("Owned fixture admission exceeded the original launch deadline")
        state["foreground_permission"] = dict(pid=pid, attempted=True, completed=False)
        launcher.allow_product(pid)
        state["foreground_permission"]["completed"] = True
        exposed = observe_exposure(native, hwnd, process, deadline)
        state["owned_foreground_unoccluded_seconds"] = exposed
        launcher.product_exposed()
        region = Region(40, 40, 2, 2)
        capture_owner = RegionCapture(hwnd, pid, region, creation_ticks=creation, storage=DxgiCopyStorage(library))
        with capture_owner as capture:
            state["initial_observations"] = []
            first = observe_palette(capture, clock, 0, 0, clock.read() + clock.frequency, state["initial_observations"])
            with RegionCapture(hwnd, pid, region, creation_ticks=creation) as gdi:
                before = [gdi.sample(), gdi.sample()]
                state["gdi_initial"] = [sample_record(item) for item in before]
                if not all(color_equal(item.bgra, palette_bgra(0)) for item in before):
                    raise RuntimeError("Initial DXGI pixels lack stable independent GDI corroboration")
                point = FixtureControl(native).point(hwnd, pid, thread)
                with NativePointer(native, hwnd, pid, clock=clock) as pointer:
                    pointer.move_screen(point)
                    input_begin = clock.read()
                    state["input_begin_ticks"] = input_begin
                    try:
                        state["input_injection"] = pointer.click()
                    finally:
                        if pointer.last_click is not None:
                            state["input_injection"] = dict(pointer.last_click)
                    state["changed_observations"] = []
                    second = observe_palette(capture, clock, 1, input_begin, input_begin + clock.frequency, state["changed_observations"])
                after = [gdi.sample(), gdi.sample()]
                state["gdi_changed"] = [sample_record(item) for item in after]
                if not all(color_equal(item.bgra, palette_bgra(1)) for item in after):
                    raise RuntimeError("Changed DXGI pixels lack stable independent GDI corroboration")
        native.post(hwnd, pid, 0x0010, 0, 0)
        output.collect("known_color_graceful_close")
        if process.returncode != 0 or output.stderr:
            raise RuntimeError("Known-color native fixture did not terminate successfully")
        state["paint_records"] = verify_paints(output.stdout, pid, hwnd, thread, clock.frequency, first, second, input_begin)
        helper.verify()
        if hashlib.sha256(executable.read_bytes()).hexdigest() != digest:
            raise RuntimeError("Known-color fixture changed during qualification")
        state["accepted"] = True
        return 0
    except BaseException as error:
        primary = error
        if hasattr(error, "state"):
            state["exposure_failure"] = error.state
        raise
    finally:
        if process is not None:
            try:
                state["process_before_cleanup"] = dict(pid=process.pid, exit_code=process.poll(),
                                                       output_complete=output.complete if output is not None else False)
            except BaseException as error:
                cleanup.append(error)
        if capture_owner is not None and isinstance(capture_owner.last_attempt, dict):
            state["capture_failure_evidence"] = dict(capture_owner.last_attempt)
            state["capture_failure_evidence"]["cleanup_evidence"] = getattr(capture_owner._storage, "last_cleanup", None)
        if output is not None and not output.complete:
            try:
                if hwnd is not None and process.poll() is None and native.owns(hwnd, process.pid):
                    native.post(hwnd, process.pid, 0x0010, 0, 0)
                    output.collect("failed_known_color_graceful_close")
            except BaseException as error:
                cleanup.append(error)
            if not output.complete:
                try:
                    output.failed_teardown()
                except BaseException as error:
                    cleanup.append(error)
        state["foreground_launch_setup"] = launcher.receipt
        try:
            launcher.__exit__(None, None, None)
        except BaseException as error:
            cleanup.append(error)
        if output is not None:
            state["process_output"] = output.receipt()
        if primary or cleanup:
            state["accepted"] = False
            state["failure_causes"] = [cause for error in ([primary] if primary else []) + cleanup for cause in error_causes(error)]
        try:
            write_json(args.output / "qualification.json", state)
        except BaseException as error:
            cleanup.append(error)
        if cleanup:
            raise BaseExceptionGroup("Known-color proof and owned teardown failed", ([primary] if primary else []) + cleanup)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--fixture-sha256", required=True)
    parser.add_argument("--capture-helper", type=Path, required=True)
    parser.add_argument("--capture-helper-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
