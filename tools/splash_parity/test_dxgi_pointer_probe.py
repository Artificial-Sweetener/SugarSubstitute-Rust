# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Exercise actual diagnostic control paths with fake external Windows boundaries."""
from __future__ import annotations
import copy
import io
import json
import tempfile
import unittest
from contextlib import ExitStack
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
try:
    from . import dxgi_pointer_probe as owner
    from .region_records import RegionSample, Geometry, Region, GdiMetadata
except ImportError:
    import dxgi_pointer_probe as owner
    from region_records import RegionSample, Geometry, Region, GdiMetadata


def readiness():
    return dict(schema_version=1, event="format_acquisition_ready", pid=41, thread=81,
                monitor=17, qpc_frequency=1000, ready_qpc=135)


def acquisition(timeout=False):
    record = {key: 0 for key in owner.ACQUISITION_KEYS}
    record.update(enroll_hresult=0, attempted=True, frame_acquired=not timeout, protected_content_masked=False,
                  acquire_hresult=-2005270489 if timeout else 0, query_hresult=1 if timeout else 0,
                  release_hresult=1 if timeout else 0, primary_hresult=-2005270489 if timeout else 0,
                  primary_operation=9 if timeout else 0, release_attempts=0 if timeout else 1,
                  timeout_ms=500, qpc_frequency=1000, acquire_begin_qpc=145, acquire_end_qpc=165,
                  last_mouse_qpc=0 if timeout else 155, last_present_qpc=0, accumulated_frames=0 if timeout else 1,
                  texture_format=0 if timeout else 10, width=0 if timeout else 1920, height=0 if timeout else 1080,
                  mip_levels=0 if timeout else 1, array_size=0 if timeout else 1, sample_count=0 if timeout else 1)
    return record


def final_record(timeout=False):
    final = {key: 0 for key in owner.FINAL_KEYS}
    final.update(schema_version=1, acquisition_mode="pointer_trigger_once", acquire_format_requested=True,
                 no_pixel_copy_or_map=True, duplication_enrollments=1, duplicate_output1_attempted=False,
                 original_duplicate_output_attempted=True, acquisition_enrollment_api="DuplicateOutput",
                 acquisition_requested_formats=[], acquisition_enrollment_flags=0, acquisition_attempts_requested=1,
                 thread=81, monitor=17, output_rect=[0, 0, 1920, 1080],
                 duplicate_output=dict(hresult=0, format=10, width=1920, height=1080, rotation=1),
                 duplicate_output1_bgra_only=dict(hresult=1, format=0, width=0, height=0, rotation=0),
                 duplicate_output1_hdr_inventory=dict(attempted=False, hresult=1, format=0, width=0, height=0, rotation=0),
                 first_before_owned_update_acquire={key: False if key in ("attempted", "frame_acquired", "protected_content_masked")
                     else 1 if key in ("acquire_hresult", "query_hresult", "release_hresult") else 0 for key in owner.ACQUISITION_KEYS},
                 format_only_acquire=acquisition(timeout),
                 owned_desktop_update=dict(requested=False, create_hresult=1, paint_hresult=1, destroy_hresult=1, unregister_hresult=1,
                    paint_count=0, paint_qpc=0, window_retained=False, class_retained=False, visibility_hresult=1,
                    visible=False, topmost=False, owned_point_witnesses=False, window_rect=[0, 0, 0, 0], client_rect=[0, 0, 0, 0],
                    foreground_before=0, foreground_enrolled=0, foreground_after=0, foreground_preserved=True,
                    gdi_flush_hresult=1, dwm_flush_hresult=1, flush_clock_hresult=0, gdi_begin_qpc=0, gdi_end_qpc=0,
                    dwm_begin_qpc=0, dwm_end_qpc=0))
    return final


class Clock:
    frequency = 1000
    def __init__(self, ticks=100):
        self.ticks = ticks
    def read(self):
        self.ticks += 10
        return self.ticks


class Process:
    def __init__(self, pid=41, raw=b""):
        self.pid, self._handle, self.returncode = pid, pid + 100, None
        self.stdout = io.BytesIO(raw)
    def poll(self):
        return self.returncode


class PointerProbeTests(unittest.TestCase):
    def test_exact_readiness_rejects_forged_identity_clock_monitor_and_future_ticks(self):
        self.assertEqual(owner.validate_ready(readiness(), 41, 17, 1000, 130, 140), readiness())
        for change in (dict(pid=42), dict(thread=0), dict(thread=True), dict(monitor=18), dict(qpc_frequency=1001),
                       dict(ready_qpc=129), dict(ready_qpc=141), dict(schema_version=True), dict(event="other"), dict(extra=1)):
            with self.subTest(change=change):
                self.assertRaises(ValueError, owner.validate_ready, dict(readiness(), **change), 41, 17, 1000, 130, 140)

    def test_payload_limits_duplicate_keys_and_extra_records_never_qualify(self):
        for raw in (b"x" * (owner.LIMIT + 1), b"\xff", b'{"pid":41,"pid":42}'):
            with self.subTest(raw=raw[:40]):
                self.assertRaises((ValueError, UnicodeError), owner.decode_records, raw)
        move = dict(completed=True, begin_qpc=150, end_qpc=160)
        for records in ([], [readiness()], [readiness(), final_record(), readiness()], [readiness(), dict(final_record(), extra=1)]):
            state = {}
            self.assertRaises(ValueError, owner.final_evidence, records, readiness(), move, 180, state)
            self.assertEqual(state["native_records"], records)

    def test_actual_texture_and_pointer_only_are_separate_from_pixel_or_performance_evidence(self):
        state = {}
        owner.final_evidence([readiness(), final_record()], readiness(), dict(completed=True, begin_qpc=150, end_qpc=160), 180, state)
        self.assertEqual(state["actual_texture"]["texture_format"], 10)
        self.assertTrue(state["pointer_trigger_attributed"])
        self.assertTrue(state["pointer_only"])
        self.assertEqual(state["movement_relation"], "overlaps_native_acquire")

    def test_before_entry_and_after_completion_keep_format_facts_without_invented_causality(self):
        for begin, end, mouse, expected, relation in ((136, 140, 138, True, "before_native_acquire"),
                                                    (166, 170, 155, False, "after_native_acquire"),
                                                    (150, 160, 149, False, "overlaps_native_acquire"),
                                                    (150, 160, 166, False, "overlaps_native_acquire")):
            with self.subTest(begin=begin, mouse=mouse):
                final = final_record()
                final["format_only_acquire"]["last_mouse_qpc"] = mouse
                state = {}
                owner.final_evidence([readiness(), final], readiness(), dict(completed=True, begin_qpc=begin, end_qpc=end), 180, state)
                self.assertEqual(state["actual_texture"]["texture_format"], 10)
                self.assertIs(state["pointer_trigger_attributed"], expected)
                self.assertEqual(state["movement_relation"], relation)

    def test_failed_movement_and_native_clock_keep_query_release_metadata(self):
        state = {}
        owner.final_evidence([readiness(), final_record()], readiness(), dict(completed=False, begin_qpc=150), 180, state)
        self.assertEqual(state["actual_texture"]["texture_format"], 10)
        self.assertFalse(state["pointer_trigger_attributed"])
        final = final_record()
        final["format_only_acquire"]["clock_hresult"] = -1
        state = {}
        self.assertRaises(ValueError, owner.final_evidence, [readiness(), final], readiness(), dict(completed=True, begin_qpc=150, end_qpc=160), 180, state)
        self.assertEqual(state["actual_texture"]["texture_format"], 10)

    def test_one_timeout_leaves_format_unknown_and_never_becomes_pointer_evidence(self):
        state = {}
        owner.final_evidence([readiness(), final_record(True)], readiness(), dict(completed=True, begin_qpc=150, end_qpc=160), 180, state)
        self.assertIsNone(state["actual_texture"])
        self.assertEqual(state["actual_format_status"], "unknown")
        self.assertFalse(state["pointer_trigger_attributed"])

    def test_native_order_clock_release_extra_acquisition_and_mode_reject(self):
        changes = [("acquire_begin_qpc", 134), ("acquire_end_qpc", 144), ("acquire_end_qpc", 181),
                   ("qpc_frequency", True), ("release_attempts", 2), ("release_hresult", -1),
                   ("timeout_ms", 1000), ("attempted", 1), ("last_mouse_qpc", 181), ("texture_format", True)]
        for key, value in changes:
            with self.subTest(key=key, value=value):
                final = final_record()
                final["format_only_acquire"][key] = value
                self.assertRaises((ValueError, RuntimeError), owner.final_evidence, [readiness(), final], readiness(),
                                  dict(completed=True, begin_qpc=150, end_qpc=160), 180, {})
        for final in (dict(final_record(), acquisition_attempts_requested=2), dict(final_record(), acquisition_mode="other"),
                      dict(final_record(), first_before_owned_update_acquire=dict(attempted=True))):
            self.assertRaises(ValueError, owner.final_evidence, [readiness(), final], readiness(), dict(completed=True, begin_qpc=150, end_qpc=160), 180, {})

    def test_retained_process_rejects_wrong_path_creation_pid_and_early_exit(self):
        process = Process()
        path = Path("probe.exe")
        for pid, actual, born, alive in ((42, path, 200, True), (41, Path("foreign.exe"), 200, True),
                                         (41, path, 201, True), (41, path, 200, False)):
            boundary = SimpleNamespace(identity=lambda _: (pid, actual, born), alive=lambda _: alive)
            self.assertRaises(RuntimeError, owner.verify_process, boundary, process, path, 200)
        process.returncode = 1
        boundary = SimpleNamespace(identity=lambda _: (41, path, 200), lifetime_identity=lambda _: (41, 200), alive=lambda _: True)
        self.assertRaises(RuntimeError, owner.verify_process, boundary, process, path, 200)
        self.assertEqual(owner.verify_process(boundary, process, path, 200, live=False), 200)

    def wait_boundary(self, *, raw=None, availability=None, exit_early=False, foreign_thread=False, ticks=None):
        raw = raw if raw is not None else json.dumps(readiness()).encode() + b"\n"
        process, events, state = Process(raw=raw), [], {}
        process.returncode = 1 if exit_early else None
        class Pipe:
            def available(self, stream):
                events.append("peek")
                return availability if availability is not None else len(stream.getbuffer()) - stream.tell()
            def verify_thread(self, thread, pid, creation):
                events.append("thread")
                if foreign_thread:
                    raise RuntimeError("foreign native thread")
                return creation
        processes = SimpleNamespace(identity=lambda _: (41, Path("probe.exe"), 200), alive=lambda _: True)
        clock = Clock(130)
        with patch.object(owner.time, "perf_counter", side_effect=ticks or (lambda: 10)), patch.object(owner.time, "sleep"):
            try:
                result = owner.wait_ready(Pipe(), processes, process, Path("probe.exe"), 200, 17, clock, 130, 15, state)
                return result, events, state
            except BaseException as error:
                return error, events, state

    def test_ready_pipe_peeks_only_available_bytes_and_records_thread_before_return(self):
        result, events, state = self.wait_boundary()
        self.assertIsInstance(result, tuple)
        self.assertEqual(events, ["peek", "thread"])
        self.assertEqual(state["readiness"]["record"], readiness())
        self.assertEqual(state["readiness"]["polls"], 1)

    def test_ready_wait_rejects_early_exit_foreign_thread_deadline_and_oversized_pipe(self):
        for options in (dict(exit_early=True), dict(foreign_thread=True), dict(ticks=lambda: 15),
                        dict(availability=owner.LIMIT + 1), dict(raw=b"x" * 1025 + b"\n")):
            with self.subTest(options=options):
                error, events, state = self.wait_boundary(**options)
                self.assertIsInstance(error, (RuntimeError, ValueError))
                if "exit_early" in options or "ticks" in options:
                    self.assertNotIn("peek", events)
                if "foreign_thread" in options:
                    self.assertEqual(state["readiness"]["record"], readiness())

    def test_ready_condition_wait_has_fixed_inventory_and_original_deadline(self):
        with patch.object(owner, "POLLS", 3):
            error, events, state = self.wait_boundary(availability=0)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(events, ["peek"] * 3)
        self.assertEqual(state["readiness"]["polls"], 3)
        error, events, _ = self.wait_boundary(ticks=iter((10, 15)).__next__)
        self.assertIsInstance(error, RuntimeError)
        self.assertEqual(events, ["peek", "thread"])

    def test_terminal_wait_never_collects_or_resets_expired_original_deadline(self):
        process = Process()
        boundary = SimpleNamespace(identity=lambda _: (41, Path("probe.exe"), 200), lifetime_identity=lambda _: (41, 200))
        state = {}
        with patch.object(owner.time, "perf_counter", return_value=15):
            self.assertRaises(RuntimeError, owner.wait_terminal, boundary, process, Path("probe.exe"), 200, 15, state)
        self.assertEqual(state["terminal_observation"]["polls"], 0)
        process.returncode = 0
        with patch.object(owner.time, "perf_counter", return_value=14):
            owner.wait_terminal(boundary, process, Path("probe.exe"), 200, 15, state)
        self.assertEqual(state["terminal_observation"], dict(polls=1, exit_code=0))

    def test_movement_keeps_native_effect_when_end_clock_fails_and_both_causes_when_move_fails(self):
        class Pointer:
            original, last = (20, 20), (20, 20)
            native = SimpleNamespace(user=SimpleNamespace(GetAsyncKeyState=lambda _: 0))
            def move_screen(self, point):
                self.last = point
            def cursor(self):
                return self.last
        inventory = []
        clock = SimpleNamespace(read=iter((150,)).__next__)
        with patch.object(owner.time, "perf_counter", return_value=10):
            self.assertRaises(BaseExceptionGroup, owner.movement, Pointer(), (300, 160), clock, 15, inventory)
        self.assertTrue(inventory[0]["completed"])
        self.assertEqual(inventory[0]["actual_cursor"], [300, 160])
        class Foreign(Pointer):
            def move_screen(self, point):
                raise RuntimeError("foreign cursor")
        inventory = []
        with patch.object(owner.time, "perf_counter", return_value=10):
            with self.assertRaises(BaseExceptionGroup) as error:
                owner.movement(Foreign(), (300, 160), SimpleNamespace(read=iter((150,)).__next__), 15, inventory)
        self.assertIn("foreign cursor", str(owner.error_causes(error.exception)))
        self.assertFalse(inventory[0]["completed"])

    def test_post_movement_ownership_failure_still_records_actual_cursor_effect(self):
        class Pointer:
            original, last = (20, 20), (20, 20)
            native = SimpleNamespace(user=SimpleNamespace(GetAsyncKeyState=lambda _: 0))
            def move_screen(self, point):
                self.last = point
                raise RuntimeError("final foreign owned-hit failure")
            def cursor(self):
                return self.last
        inventory = []
        with patch.object(owner.time, "perf_counter", return_value=10):
            self.assertRaises(BaseExceptionGroup, owner.movement, Pointer(), (300, 160), Clock(), 15, inventory)
        self.assertFalse(inventory[0]["completed"])
        self.assertEqual(inventory[0]["actual_cursor"], [300, 160])
        self.assertEqual(inventory[0]["owner_last_position"], [300, 160])
        self.assertEqual(inventory[0]["insertion_result"], "Not exposed by the common pointer owner")

    def test_late_human_button_disables_cursor_restoration_and_keeps_primary(self):
        for key in (1, 2, 4, 5, 6):
            events, held = [], []
            native = SimpleNamespace(user=SimpleNamespace(GetAsyncKeyState=lambda observed: 0x8000 if observed in held else 0))
            class Pointer:
                original = (20, 20)
                def __init__(self, *args):
                    pass
                def __enter__(self):
                    return self
                def __exit__(self, *_):
                    events.append(self.original)
            state = {}
            with patch.object(owner, "NativePointer", Pointer):
                with self.assertRaises(BaseExceptionGroup) as failure:
                    with owner.pointer_lifetime(native, 11, 55, state):
                        held.append(key)
                        raise RuntimeError("native terminal failure")
            self.assertEqual(events, [None])
            self.assertIn("native terminal failure", str(owner.error_causes(failure.exception)))
            self.assertIn("cursor_restoration_suppressed", state)

    def run_boundary(self, *, forged=False, move_failure=False, collect_human=False, close_failure=False,
                     terminal_clock_failure=False, second_read_failure=False, gdi_results=None,
                     gdi_expire_after=None, gdi_geometry=None, exited_image_failure=False,
                     terminal_identity_failure=False, actual_inactive_foreground=False):
        events, processes, held, clock, elapsed = [], {}, [], Clock(), [10]
        if terminal_clock_failure:
            original_read = clock.read
            def failing_read():
                if clock.ticks >= 160:
                    raise RuntimeError("terminal observer clock failure")
                return original_read()
            clock.read = failing_read
        native = SimpleNamespace()
        native.user = SimpleNamespace(GetAsyncKeyState=lambda key: 0x8000 if key in held else 0,
                                      IsWindowVisible=lambda _: True, MonitorFromPoint=lambda *_: 17)
        def window_thread(hwnd, target):
            target._obj.value = 55
            return 71
        native.user.GetWindowThreadProcessId = window_thread
        native.owns = lambda hwnd, pid: hwnd == 11 and pid == 55
        native.find_window = lambda process, deadline: events.append("find") or 11
        native.post = lambda *args: events.append("close_fixture")
        class Launcher:
            receipt = {}
            def __init__(self, *args):
                pass
            def __enter__(self):
                events.append("launcher")
            def allow_product(self, pid):
                events.append("permission")
            def product_exposed(self):
                events.append("exposed")
            def __exit__(self, *_):
                events.append("launcher_close")
        class Capture:
            calls = 0
            def __init__(self, *args, **options):
                pass
            def __enter__(self):
                if gdi_expire_after == 0:
                    elapsed[0] = 15
                return self
            def sample(self):
                events.append("gdi")
                self.calls += 1
                if second_read_failure and self.calls == 2:
                    raise RuntimeError("second GDI sample failure")
                if gdi_expire_after == self.calls:
                    elapsed[0] = 15
                pixels = gdi_results[min(self.calls - 1, len(gdi_results) - 1)] if gdi_results else owner.palette_bgra(0)
                geometry = gdi_geometry[min(self.calls - 1, len(gdi_geometry) - 1)] if gdi_geometry else Geometry(558, 558, 96, 100, 100)
                return RegionSample(pixels, 11, 55, 100, geometry, Region(40, 40, 2, 2),
                                    1000, 90, 90, 90, 95, 95, GdiMetadata(None, None, 90, 95, 95), "gdi")
            def __exit__(self, *_):
                events.append("capture_close")
        class Pointer:
            original, last, moves = (20, 20), (20, 20), 0
            def __init__(self, *args):
                self.native = native
            def __enter__(self):
                return self
            def cursor(self):
                return self.last
            def move_screen(self, point):
                self.moves += 1
                events.append("movement")
                if move_failure and self.moves == 2:
                    raise RuntimeError("foreign movement failure")
                self.last = point
                if self.moves == 2:
                    processes[41].returncode = 0
            def __exit__(self, *_):
                events.append("restore" if self.original is not None else "preserve_human")
        class Pipe:
            def __init__(self, *args):
                pass
            def available(self, stream):
                return len(json.dumps(dict(readiness(), monitor=18) if forged else readiness()).encode()) + 1
            def verify_thread(self, *args):
                events.append("thread_verified")
                return 200
            def close(self):
                events.append("pipe_close")
                if close_failure:
                    raise RuntimeError("thread close failure")
        class Output:
            complete, stdout, stderr = False, b"", b""
            def __init__(self, process, output):
                self.process = process
            def collect(self, phase):
                events.append(phase)
                self.stdout = self.process.stdout.read()
                self.complete, self.process.returncode = True, 0
                if collect_human and self.process.pid == 41:
                    held.append(2)
            def failed_teardown(self):
                events.append(f"forced_{self.process.pid}")
                self.stdout = self.process.stdout.read()
                self.complete, self.process.returncode = True, 1
            def receipt(self):
                return dict(exit_code=self.process.returncode, complete=self.complete)
        with tempfile.TemporaryDirectory() as directory:
            paths = {name: Path(directory) / f"{name}.exe" for name in ("fixture", "probe")}
            for name, path in paths.items():
                path.write_bytes(name.encode())
            args = SimpleNamespace(fixture=paths["fixture"], probe=paths["probe"],
                                   fixture_sha256=owner.hashlib.sha256(b"fixture").hexdigest(),
                                   probe_sha256=owner.hashlib.sha256(b"probe").hexdigest(), output=Path(directory) / "out")
            def launch(command, **options):
                name = Path(command[0]).stem
                events.append(f"launch_{name}")
                pid = 55 if name == "fixture" else 41
                final = final_record()
                if actual_inactive_foreground:
                    final["owned_desktop_update"].update(foreground_after=100075508, foreground_preserved=False)
                raw = b"fixture metadata\n" if pid == 55 else (json.dumps(dict(readiness(), monitor=18) if forged else readiness()) + "\n" + json.dumps(final) + "\n").encode()
                process = Process(pid, raw)
                processes[pid] = process
                return process
            def identity(handle):
                pid = handle - 100
                if processes[pid].returncode is not None:
                    events.append("exited_image_query")
                    if exited_image_failure:
                        raise OSError("exited process image unavailable")
                return pid, paths["fixture" if pid == 55 else "probe"], 100 if pid == 55 else 200
            def lifetime_identity(handle):
                events.append("lifetime_identity")
                pid = handle - 100
                if terminal_identity_failure and pid == 41:
                    raise OSError("retained native lifetime query failed")
                return pid, 100 if pid == 55 else 200
            boundary = SimpleNamespace(kernel=object(), identity=identity, lifetime_identity=lifetime_identity,
                                       alive=lambda handle: processes[handle - 100].returncode is None)
            with ExitStack() as stack:
                for key, value in {"NativeWindows": lambda: native, "ReferenceProcesses": lambda: boundary,
                                   "Qpc": lambda: clock, "ForegroundLauncher": Launcher, "ProbePipe": Pipe,
                                   "OperationalOutput": Output, "RegionCapture": Capture, "NativePointer": Pointer,
                                   "FixtureControl": lambda _: SimpleNamespace(point=lambda *_: (300, 160)),
                                   "observe_exposure": lambda *args: 10}.items():
                    stack.enter_context(patch.object(owner, key, value))
                stack.enter_context(patch.object(owner.subprocess, "Popen", launch))
                stack.enter_context(patch.object(owner.time, "perf_counter", side_effect=lambda: elapsed[0]))
                stack.enter_context(patch.object(owner.time, "sleep"))
                # External available-pipe boundary returns one line; no Windows calls.
                try:
                    result, failure = owner.run(args), None
                except BaseException as error:
                    result, failure = None, error
            saved = json.loads((args.output / "observation.json").read_text())
            raw_path = args.output / "probe-stdout.bin"
            raw = raw_path.read_bytes() if raw_path.exists() else b""
            return result, failure, saved, events, raw

    def test_full_controller_requires_exposure_two_gdi_palettes_then_one_probe_and_trigger(self):
        result, failure, state, events, raw = self.run_boundary()
        self.assertIsNone(failure)
        self.assertEqual(result, 0)
        self.assertEqual(events.count("gdi"), 2)
        self.assertEqual(events.count("movement"), 2)
        self.assertLess(events.index("find"), events.index("permission"))
        self.assertLess(events.index("gdi"), events.index("launch_probe"))
        self.assertLess(events.index("thread_verified"), len(events) - 1)
        self.assertEqual(len(owner.decode_records(raw)), 2)
        self.assertTrue(state["experiment_completed"])
        self.assertTrue(state["pointer_trigger_attributed"])
        self.assertTrue(state["pointer_only"])
        self.assertFalse(state["pixel_accepted"])
        self.assertFalse(state["performance_accepted"])
        self.assertEqual(state["actual_texture"]["texture_format"], 10)
        self.assertIn("close_fixture", events)
        self.assertIn("pipe_close", events)
        self.assertIn("launcher_close", events)

    def test_forged_readiness_never_dispatches_trigger_and_keeps_raw_native_metadata_and_cleanup(self):
        _, failure, state, events, raw = self.run_boundary(forged=True)
        self.assertIsNotNone(failure)
        self.assertEqual(events.count("movement"), 1)
        self.assertFalse(state["experiment_completed"])
        self.assertEqual(len(owner.decode_records(raw)), 2)
        self.assertIn("forced_41", events)
        self.assertIn("close_fixture", events)
        self.assertIsNone(state["processes"]["probe"]["before_cleanup"]["exit_code"])
        self.assertEqual(state["processes"]["probe"]["output"]["exit_code"], 1)

    def test_failed_trigger_keeps_actual_format_and_native_raw_causes(self):
        _, failure, state, events, raw = self.run_boundary(move_failure=True)
        self.assertIsNotNone(failure)
        self.assertIn("foreign movement failure", str(state["failure_causes"]))
        self.assertFalse(state["pointer_trigger_attributed"])
        self.assertEqual(state["actual_texture"]["texture_format"], 10)
        self.assertEqual(len(owner.decode_records(raw)), 2)
        self.assertIn("forced_41", events)

    def test_human_button_during_terminal_collect_preserves_cursor_and_actual_format(self):
        _, failure, state, events, _ = self.run_boundary(collect_human=True)
        self.assertIsNotNone(failure)
        self.assertIn("preserve_human", events)
        self.assertNotIn("restore", events)
        self.assertFalse(state["pointer_trigger_attributed"])
        self.assertEqual(state["actual_texture"]["texture_format"], 10)
        self.assertIn("cursor_restoration_suppressed", state)

    def test_primary_trigger_and_close_failure_are_retained_together(self):
        _, failure, state, events, _ = self.run_boundary(move_failure=True, close_failure=True)
        self.assertIsInstance(failure, BaseExceptionGroup)
        self.assertIn("foreign movement failure", str(state["failure_causes"]))
        self.assertIn("thread close failure", str(state["failure_causes"]))
        self.assertIn("launcher_close", events)

    def test_repeated_terminal_clock_failure_cannot_discard_acquired_query_release_facts(self):
        _, failure, state, events, raw = self.run_boundary(terminal_clock_failure=True)
        self.assertIsNotNone(failure)
        self.assertEqual(len(owner.decode_records(raw)), 2)
        self.assertEqual(state["actual_texture"]["texture_format"], 10)
        self.assertEqual(state["actual_format_status"], "acquired and queried and released")
        self.assertFalse(state["experiment_completed"])
        self.assertFalse(state["pointer_trigger_attributed"])
        self.assertIn("terminal observer clock failure", str(state["failure_causes"]))
        self.assertIn("close_fixture", events)

    def test_second_gdi_failure_retains_first_actual_palette_without_probe_or_movement(self):
        _, failure, state, events, raw = self.run_boundary(second_read_failure=True)
        self.assertIsNotNone(failure)
        self.assertEqual(len(state["initial_gdi"]), 1)
        self.assertEqual(state["initial_gdi"][0]["bgra"], owner.palette_bgra(0).hex())
        self.assertNotIn("launch_probe", events)
        self.assertNotIn("movement", events)
        self.assertFalse(state["experiment_completed"])
        self.assertEqual(raw, b"")
        self.assertIn("second GDI sample failure", str(state["failure_causes"]))
        self.assertIn("capture_close", events)
        self.assertIn("close_fixture", events)

    def test_terminal_retained_lifetime_never_queries_exited_image(self):
        _, failure, state, events, raw = self.run_boundary(exited_image_failure=True, actual_inactive_foreground=True)
        self.assertIsNone(failure)
        self.assertTrue(state["experiment_completed"])
        self.assertIn("lifetime_identity", events)
        self.assertNotIn("exited_image_query", events)
        self.assertEqual(owner.decode_records(raw)[1]["owned_desktop_update"]["foreground_after"], 100075508)

    def test_terminal_native_lifetime_failure_rejects_and_retains_raw_completed_metadata(self):
        _, failure, state, events, raw = self.run_boundary(terminal_identity_failure=True)
        self.assertIsNotNone(failure)
        self.assertIn("retained native lifetime query failed", str(state["failure_causes"]))
        self.assertFalse(state["experiment_completed"])
        self.assertEqual(len(owner.decode_records(raw)), 2)
        self.assertEqual(state["actual_format_status"], "acquired and queried and released")
        self.assertNotIn("exited_image_query", events)

    def test_terminal_identity_requires_verified_creation_and_rejects_bad_pid_or_creation(self):
        process = Process()
        boundary = SimpleNamespace(lifetime_identity=lambda _: (41, 200))
        self.assertRaises(ValueError, owner.verify_process, boundary, process, Path("probe.exe"), live=False)
        for actual in ((42, 200), (41, 201), (True, 200), (41, False), (41, 200.0), (41, 0)):
            boundary.lifetime_identity = lambda _, value=actual: value
            with self.subTest(actual=actual):
                self.assertRaises((ValueError, RuntimeError), owner.verify_process, boundary, process, Path("probe.exe"), 200, live=False)

    def test_actual_inactive_update_foreground_observation_preserves_timeout_unknown(self):
        final = final_record(True)
        # Exact first-party producer observation from owned native03; no update work occurred.
        final["owned_desktop_update"].update(foreground_after=100075508, foreground_preserved=False)
        state = {}
        owner.final_evidence([readiness(), final], readiness(), dict(completed=True, begin_qpc=150, end_qpc=160), 180, state)
        self.assertIsNone(state["actual_texture"])
        self.assertEqual(state["actual_format_status"], "unknown")
        self.assertFalse(state["pointer_trigger_attributed"])
        self.assertFalse(state["pointer_only"])
        for after, preserved in ((0, True), (1, False), (100075508, False)):
            final["owned_desktop_update"].update(foreground_after=after, foreground_preserved=preserved)
            owner.final_evidence([readiness(), final], readiness(), dict(completed=True, begin_qpc=150, end_qpc=160), 180, {})
        for after, preserved in ((0, False), (100075508, True), (True, False), (-1, False), (1 << 64, False), (1.0, False), (1, 0)):
            final["owned_desktop_update"].update(foreground_after=after, foreground_preserved=preserved)
            with self.subTest(after=after, preserved=preserved):
                self.assertRaises(ValueError, owner.final_evidence, [readiness(), final], readiness(),
                                  dict(completed=True, begin_qpc=150, end_qpc=160), 180, {})

    def test_initial_palette_retains_mismatches_until_two_consecutive_exact_observations(self):
        expected, mismatch = owner.palette_bgra(0), bytes(16)
        _, failure, state, events, _ = self.run_boundary(gdi_results=[mismatch, mismatch, expected, expected])
        self.assertIsNone(failure)
        self.assertEqual([sample["bgra"] for sample in state["initial_gdi"]],
                         [value.hex() for value in [mismatch, mismatch, expected, expected]])
        self.assertEqual(state["initial_palette_readiness"],
                         dict(attempts=4, limit=256, consecutive=2, completed=True))
        self.assertEqual(events[:events.index("launch_probe")].count("gdi"), 4)

    def test_initial_palette_mismatch_resets_consecutive_success_count(self):
        expected, mismatch = owner.palette_bgra(0), bytes(16)
        _, failure, state, events, _ = self.run_boundary(gdi_results=[expected, mismatch, expected, expected])
        self.assertIsNone(failure)
        self.assertEqual(len(state["initial_gdi"]), 4)
        self.assertEqual(events[:events.index("movement")].count("gdi"), 4)
        self.assertTrue(state["initial_palette_readiness"]["completed"])

    def test_initial_palette_consecutive_samples_require_identical_geometry(self):
        first, changed = Geometry(558, 558, 96, 101, 100), Geometry(558, 558, 96, 100, 100)
        _, failure, state, events, _ = self.run_boundary(gdi_geometry=[first, changed, changed])
        self.assertIsNone(failure)
        self.assertEqual(events.count("gdi"), 3)
        self.assertEqual(state["initial_palette_readiness"]["attempts"], 3)

    def test_initial_palette_original_deadline_rejects_before_or_after_native_read(self):
        for expiry, retained in [(0, 0), (1, 1), (2, 2)]:
            with self.subTest(expiry=expiry):
                _, failure, state, events, _ = self.run_boundary(gdi_expire_after=expiry)
                self.assertIsNotNone(failure)
                self.assertEqual(state["launch_deadline_seconds"], 15)
                self.assertEqual(len(state["initial_gdi"]), retained)
                self.assertEqual(events.count("gdi"), retained)
                self.assertFalse(state["initial_palette_readiness"]["completed"])
                self.assertNotIn("movement", events)
                self.assertNotIn("launch_probe", events)
                self.assertIn("original launch deadline", str(state["failure_causes"]))
                self.assertIn("capture_close", events)
                self.assertIn("close_fixture", events)

    def test_initial_palette_inventory_exhausts_without_probe_or_input(self):
        _, failure, state, events, _ = self.run_boundary(gdi_results=[bytes(16)])
        self.assertIsNotNone(failure)
        self.assertEqual(len(state["initial_gdi"]), 256)
        self.assertEqual(events.count("gdi"), 256)
        self.assertEqual(state["initial_palette_readiness"],
                         dict(attempts=256, limit=256, consecutive=0, completed=False))
        self.assertNotIn("movement", events)
        self.assertNotIn("launch_probe", events)
        self.assertIn("inventory", str(state["failure_causes"]))

    def test_failed_read_after_mismatch_retains_actual_pixels_and_attempt_count(self):
        _, failure, state, events, _ = self.run_boundary(gdi_results=[bytes(16)], second_read_failure=True)
        self.assertIsNotNone(failure)
        self.assertEqual([sample["bgra"] for sample in state["initial_gdi"]], [bytes(16).hex()])
        self.assertEqual(state["initial_palette_readiness"],
                         dict(attempts=2, limit=256, consecutive=0, completed=False))
        self.assertNotIn("movement", events)
        self.assertNotIn("launch_probe", events)

    def test_bad_executable_enrollment_precedes_any_native_controller_effect(self):
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "probe.exe"
            executable.write_bytes(b"exact executable")
            digest = owner.hashlib.sha256(executable.read_bytes()).hexdigest()
            self.assertEqual(owner.enroll(executable, digest), executable)
            for path, expected in ((Path("relative.exe"), digest), (executable, "0" * 64),
                                   (executable, "+" + "0" * 63), (executable, " " + "0" * 63)):
                self.assertRaises(ValueError, owner.enroll, path, expected)
            args = SimpleNamespace(fixture=executable, fixture_sha256="0" * 64, probe=executable,
                                   probe_sha256=digest, output=Path(directory) / "out")
            with patch.object(owner, "NativeWindows") as native, patch.object(owner.subprocess, "Popen") as launch:
                self.assertRaises(ValueError, owner.run, args)
            native.assert_not_called()
            launch.assert_not_called()
            self.assertFalse(args.output.exists())

    def test_actual_thread_query_handle_closes_once_and_retains_failed_cleanup_for_retry(self):
        events, failing_close = [], [True]
        class Call:
            def __init__(self, function):
                self.function = function
            def __call__(self, *args):
                return self.function(*args)
        def stamps(handle, *targets):
            targets[0]._obj.dwLowDateTime = 200
            return True
        def close(handle):
            events.append(("close", handle))
            return not failing_close[0]
        kernel = SimpleNamespace(PeekNamedPipe=Call(lambda *_: True), OpenThread=Call(lambda *args: events.append(("open", args)) or 93),
                                 GetProcessIdOfThread=Call(lambda _: 41), GetThreadTimes=Call(stamps), CloseHandle=Call(close))
        pipe = owner.ProbePipe(kernel)
        self.assertEqual(pipe.verify_thread(81, 41, 200), 200)
        self.assertEqual(events[0], ("open", (0x0800, False, 81)))
        with patch.object(owner.ctypes, "WinError", side_effect=lambda code: OSError(code, "native close error"), create=True), \
                patch.object(owner.ctypes, "get_last_error", return_value=6, create=True):
            self.assertRaises(OSError, pipe.close)
        self.assertEqual(pipe.thread_handle, 93)
        self.assertIs(owner._THREAD_OWNERS[id(pipe)], pipe)
        failing_close[0] = False
        pipe.close()
        pipe.close()
        self.assertIsNone(pipe.thread_handle)
        self.assertNotIn(id(pipe), owner._THREAD_OWNERS)
        self.assertEqual(events.count(("close", 93)), 2)

    def test_thread_process_or_creation_failure_keeps_actual_handle_until_cleanup(self):
        class Call:
            def __init__(self, function):
                self.function = function
            def __call__(self, *args):
                return self.function(*args)
        for actual_pid, born in ((42, 200), (41, 199)):
            def stamps(handle, *targets):
                targets[0]._obj.dwLowDateTime = born
                return True
            released = []
            kernel = SimpleNamespace(PeekNamedPipe=Call(lambda *_: True), OpenThread=Call(lambda *_: 93),
                                     GetProcessIdOfThread=Call(lambda _: actual_pid), GetThreadTimes=Call(stamps),
                                     CloseHandle=Call(lambda handle: released.append(handle) or True))
            pipe = owner.ProbePipe(kernel)
            self.assertRaises(RuntimeError, pipe.verify_thread, 81, 41, 200)
            self.assertEqual(pipe.thread_handle, 93)
            self.assertIs(owner._THREAD_OWNERS[id(pipe)], pipe)
            pipe.close()
            self.assertEqual(released, [93])

    def test_invalid_nested_descriptor_dimensions_and_extra_native_effects_reject(self):
        for change in (lambda final: final.update(output_rect=[0, 0, 0, 1080]),
                       lambda final: final.update(color_space=True),
                       lambda final: final["duplicate_output"].update(extra=1),
                       lambda final: final["duplicate_output1_hdr_inventory"].update(attempted=True),
                       lambda final: final["format_only_acquire"].update(width=0),
                       lambda final: final["first_before_owned_update_acquire"].update(frame_acquired=True),
                       lambda final: final["owned_desktop_update"].update(requested=True)):
            final = final_record()
            change(final)
            state = {}
            self.assertRaises(ValueError, owner.final_evidence, [readiness(), final], readiness(),
                              dict(completed=True, begin_qpc=150, end_qpc=160), 180, state)
            self.assertEqual(state["native_records"][1], final)

    def test_live_nonterminal_probe_fails_fixed_terminal_inventory_without_fresh_wait_budget(self):
        process, state = Process(), {}
        boundary = SimpleNamespace(identity=lambda _: (41, Path("probe.exe"), 200), lifetime_identity=lambda _: (41, 200))
        with patch.object(owner, "POLLS", 3), patch.object(owner.time, "perf_counter", return_value=10), patch.object(owner.time, "sleep") as schedule:
            self.assertRaises(RuntimeError, owner.wait_terminal, boundary, process, Path("probe.exe"), 200, 15, state)
        self.assertEqual(state["terminal_observation"], dict(polls=3, exit_code=None))
        self.assertEqual(schedule.call_count, 3)

    def test_inactive_native_records_reject_every_unknown_effect_and_scalar_type(self):
        for section in ("first_before_owned_update_acquire", "owned_desktop_update"):
            original = final_record()[section]
            changes = [dict(original, extra=1)]
            changes.append({key: value for key, value in original.items() if key != next(iter(original))})
            for key, value in original.items():
                candidate = copy.deepcopy(original)
                candidate[key] = (1 if type(value) is bool else False if type(value) is int else [False, 0, 0, 0])
                changes.append(candidate)
                if type(value) is int:
                    changes.append(dict(original, **{key: value + 1}))
            for malformed in changes:
                with self.subTest(section=section, malformed=malformed):
                    final = final_record()
                    final[section] = malformed
                    self.assertRaises(ValueError, owner.final_evidence, [readiness(), final], readiness(),
                                      dict(completed=True, begin_qpc=150, end_qpc=160), 180, {})

    def test_descriptor_enrollment_cannot_contradict_exact_one_original_duplication(self):
        for section, key, value in (("duplicate_output", "hresult", 1),
                                    ("duplicate_output1_bgra_only", "hresult", 0),
                                    ("duplicate_output1_bgra_only", "width", 1920),
                                    ("duplicate_output1_hdr_inventory", "format", 10),
                                    ("duplicate_output1_hdr_inventory", "height", 1080)):
            final = final_record()
            final[section][key] = value
            state = {}
            self.assertRaises(ValueError, owner.final_evidence, [readiness(), final], readiness(),
                              dict(completed=True, begin_qpc=150, end_qpc=160), 180, state)
            self.assertEqual(state["actual_texture"]["texture_format"], 10)
            self.assertEqual(state["native_records"][1], final)


if __name__ == "__main__":
    unittest.main()
