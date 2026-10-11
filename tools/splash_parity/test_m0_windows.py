# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Boundary fixtures exercise actual observer logic without claiming Windows native proof."""
from __future__ import annotations

import tempfile
import unittest
from contextlib import ExitStack
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

try:
    from . import m0_windows as owner
    from .region_records import RegionSample, RegionPending, Geometry, Region, GdiMetadata, DxgiOutput, DxgiMetadata, Interval
except ImportError:
    import m0_windows as owner
    from region_records import RegionSample, RegionPending, Geometry, Region, GdiMetadata, DxgiOutput, DxgiMetadata, Interval


class Clock:
    frequency = 1000000
    def __init__(self):
        self.ticks = 1000
    def read(self):
        self.ticks += 100
        return self.ticks


class Capture:
    def __init__(self, clock, cost=1000):
        self.clock, self.cost, self.expanded, self.pressed = clock, cost, False, False
        self.foreign, self.late, self.stuck, self.wrong_frequency = False, False, False, False
        self.calls = 0
    def sample(self, *, deadline_ticks, minimum_present_ticks=0):
        self.calls += 1
        if self.foreign and self.pressed:
            raise RuntimeError("Foreign ROI ownership")
        begin = self.clock.read()
        self.clock.ticks += 1000000 if self.late and self.pressed else self.cost
        end = self.clock.read()
        color = 180 if self.expanded and not self.stuck else 30
        raw = bytes([color, color, color, 0]) * 4
        return RegionSample(raw, 11, 55, 99, Geometry(558, 558, 96, 100, 100), Region(30, 400, 2, 2),
                            self.clock.frequency + int(self.wrong_frequency), begin, begin, begin, end, end,
                            GdiMetadata(None, None, begin, end, end), "gdi")


class Disclosure:
    def __init__(self, capture):
        self.capture, self.clock, self.calls = capture, capture.clock, 0
        self.pending = 0
    def read(self, deadline):
        begin = self.clock.read()
        self.clock.ticks += 100
        end = self.clock.read()
        self.calls += 1
        label = owner.LABELS[int(self.capture.expanded)]
        if self.capture.pressed and self.pending:
            self.pending -= 1
            label = owner.LABELS[1 - int(self.capture.expanded)]
        return dict(label=label, begin_ticks=begin, end_ticks=end, qpc_frequency=self.clock.frequency)


def dxgi_observation(sample, pending=False, source=None):
    """External copy-boundary fixture; shared native guards have separate real-owner tests."""
    roi = (sample.geometry.screen_x + sample.region.x, sample.geometry.screen_y + sample.region.y,
           sample.geometry.screen_x + sample.region.x + 2, sample.geometry.screen_y + sample.region.y + 2)
    output = DxgiOutput(7, 0, sample.qpc_frequency, 1, 21, 4, 0, (-1920, 0, 1920, 1080), roi,
                        1, 0, 8, 0, 87, 3840, 1080, 2, 2, "DISPLAY1", "Fixture adapter", "SDR external boundary fixture")
    stage = Interval(sample.begin_ticks, sample.begin_ticks)
    metadata = DxgiMetadata(output, "fence_pending" if pending else "ready", Interval(sample.begin_ticks, sample.storage_end_ticks),
                            1, sample.begin_ticks if source is None else source, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            stage, stage, stage, Interval(0, 0) if pending else stage, Interval(0, 0) if pending else stage)
    fields = {name: getattr(sample, name) for name in ("hwnd", "pid", "creation_ticks", "geometry", "region", "qpc_frequency",
                                                     "begin_ticks", "ownership_before_end_ticks", "storage_begin_ticks", "storage_end_ticks", "end_ticks")}
    fields.update(metadata=metadata, backend="dxgi_v1")
    return RegionPending(**fields) if pending else RegionSample(bgra=sample.bgra, **fields)


class Pointer:
    def __init__(self, capture):
        self.capture, self.events = capture, []
    def press(self):
        self.events.append("press")
        self.capture.pressed = True
    def release(self):
        self.events.append("release")
        self.capture.expanded = not self.capture.expanded
    def click(self):
        self.events.append("click")
        self.capture.pressed = True
        self.capture.expanded = not self.capture.expanded
        return dict(inserted_events=2, event_flags=[2, 4], prechecked=True, postchecked=True)
    def move_screen(self, point):
        self.point = point
    def cursor(self):
        return self.point


class Observer:
    def __init__(self):
        self.active, self.current = True, (0, 0)
        self.calls, self.foreign_after, self.wrong_position = 0, False, False
    def in_move_loop(self, *_):
        self.calls += 1
        return self.active and not (self.foreign_after and self.calls % 2 == 0)
    def response(self, *_):
        return 0.1
    def origin(self, *_):
        return self.current if not self.wrong_position else (900, 900)


def campaign_fixture():
    callbacks = [dict(sequence=index, kind=kind, begin_ticks=100 + index * 20,
                      end_ticks=110 + index * 20, authored_ticks=10, delegated_drag=None)
                 for index, kind in enumerate(("window_event", "completion_drain", "redraw", "loop_progress"), 1)]
    callbacks.append(dict(sequence=5, kind="windows_parent", begin_ticks=200, end_ticks=1000000,
                          authored_ticks=100, delegated_drag=dict(begin_ticks=250, end_ticks=999950)))
    raw = dict(schema_version=1, source="windows_qpc", begin_ticks=100, end_ticks=2000000,
               frequency=1000000, ui_thread_id=71, callbacks_observed=5, callback_samples_omitted=0,
               submissions_observed=33, submission_samples_omitted=0, callbacks=callbacks,
               submissions=[dict(sequence=i, begin_ticks=1000 + i * 16000, end_ticks=1100 + i * 16000,
                                 retained=False, identity=dict(surface=1, state_generation=1, sequence=i))
                            for i in range(1, 34)])
    drag = dict(samples=[dict(begin_ticks=10000 + i * 1000, end_ticks=10100 + i * 1000, accepted=True,
                              owned_move_loop_before=True, owned_move_loop_after=True) for i in range(32)])
    inputs = dict(samples=[dict(accepted=True) for _ in range(32)])
    return raw, drag, inputs


class M0WindowsTests(unittest.TestCase):
    def test_busy_input_preserves_every_raw_sample_and_strict_flag(self):
        reports = []
        for profile in ("strict", "busy"):
            clock = Clock()
            capture = Capture(clock, cost=60000)
            reports.append(owner.input_samples(Pointer(capture), capture, clock, 32, [], Disclosure(capture), profile=profile))
        strict, busy = reports
        self.assertFalse(strict["p95_below_50ms"])
        self.assertFalse(busy["p95_below_50ms"])
        self.assertTrue(busy["timing_assessment"]["accepted"])
        self.assertFalse(busy["timing_assessment"]["strict_passed"])
        self.assertEqual(strict["samples"], busy["samples"])
        self.assertEqual(strict["input_to_visible"], busy["input_to_visible"])
        self.assertNotIn("timing_assessment", strict)

    def test_busy_never_accepts_missing_pixels_or_unknown_profile(self):
        clock = Clock()
        capture = Capture(clock)
        capture.stuck = True
        self.assertRaises(RuntimeError, owner.input_samples, Pointer(capture), capture, clock, 32, [], Disclosure(capture), profile="busy")
        before = capture.calls
        self.assertRaises(ValueError, owner.input_samples, Pointer(capture), capture, clock, 32, [], Disclosure(capture), profile="unknown")
        self.assertEqual(capture.calls, before)
        with patch.object(owner, "enroll_helper") as helper, patch.object(owner, "NativeWindows") as native:
            self.assertRaises(ValueError, owner.run, SimpleNamespace(timing_profile="unknown"))
        helper.assert_not_called()
        native.assert_not_called()

    def test_busy_run_propagates_profile_without_weakening_campaign_or_cleanup(self):
        from unittest.mock import MagicMock
        for profile, missing in (("strict", False), ("busy", False), ("busy", True)):
            with self.subTest(profile=profile, missing=missing), tempfile.TemporaryDirectory() as directory:
                executable = Path(directory) / "fixture.exe"
                executable.write_bytes(b"exact fixture")
                raw, drag, _ = campaign_fixture()
                for index, sample in enumerate(raw["submissions"], 1):
                    sample.update(begin_ticks=1000 + index * 30000, end_ticks=1100 + index * 30000)
                if missing:
                    raw["callbacks"][0]["kind"] = "other"
                process = SimpleNamespace(pid=55, _handle=10, returncode=None, poll=lambda: process.returncode)
                clock = Clock()
                capture, pointer, disclosure = Capture(clock, cost=60000), None, None
                capture.last_attempt = None
                pointer, disclosure = Pointer(capture), Disclosure(capture)
                contexts = []
                for value in (capture, pointer, disclosure):
                    context = MagicMock()
                    context.__enter__.return_value = value
                    contexts.append(context)
                pointer.move = lambda point: None
                events = []
                class Output:
                    complete = False
                    def __init__(self, *args):
                        self.target = args[1] / "terminal.json"
                    def collect(self, phase):
                        events.append(phase)
                        process.returncode, self.complete = 0, True
                        self.target.write_text(owner.json.dumps(dict(frames_presented=33, native_timings=raw)))
                    def receipt(self):
                        return dict(complete=self.complete, exit_code=process.returncode)
                def thread(hwnd, pid):
                    pid._obj.value = 55
                    return 71
                native = SimpleNamespace(user=SimpleNamespace(GetWindowThreadProcessId=thread), find_window=lambda *_: 11,
                                         post=lambda *_: events.append("WM_CLOSE"), client=lambda *_: SimpleNamespace(scale=1))
                processes = SimpleNamespace(identity=lambda *_: (55, executable.resolve(), 99), alive=lambda *_: True)
                args = SimpleNamespace(executable=executable, output=Path(directory) / "out", workers=0, affinity_cpus=None,
                                       timing_profile=profile)
                with ExitStack() as stack:
                    for name, boundary in (("NativeWindows", lambda: native), ("ReferenceProcesses", lambda: processes),
                                           ("Qpc", lambda: clock), ("ObservationWindows", lambda *_: None),
                                           ("OperationalOutput", Output), ("observe_exposure", lambda *_: None),
                                           ("physical_drag", lambda *_: drag), ("RegionCapture", lambda *a, **k: contexts[0]),
                                           ("NativePointer", lambda *a, **k: contexts[1]), ("DisclosureLabel", lambda *_: contexts[2])):
                        stack.enter_context(patch.object(owner, name, boundary))
                    stack.enter_context(patch.object(owner.subprocess, "Popen", return_value=process))
                    result = owner.run(args)
                saved = owner.json.loads((args.output / "observation.json").read_text())
                self.assertEqual(result, 0 if profile == "busy" and not missing else 1)
                self.assertFalse(saved["input"]["p95_below_50ms"])
                self.assertFalse(saved["native_timings"]["producer_p95_below_25ms"])
                self.assertEqual(len(saved["input_samples"]), 32)
                self.assertEqual(events, ["WM_CLOSE", "graceful_close"])
                if profile == "busy":
                    self.assertFalse(saved["strict_qualification_accepted"])

    def test_busy_failed_teardown_cannot_qualify(self):
        with tempfile.TemporaryDirectory() as directory:
            _, error, receipt = self.failed_run(directory, profile="busy", collect_failure=True)
        self.assertIsInstance(error, BaseExceptionGroup)
        self.assertFalse(receipt["qualification_accepted"])
        self.assertEqual(receipt["timing_profile"], "busy")

    def test_complete_campaign_requires_new_intervals_categories_and_actual_delegated_overlap(self):
        raw, drag, inputs = campaign_fixture()
        result = owner.campaign_decision(raw, drag, inputs)
        self.assertTrue(result["accepted"])
        self.assertEqual(result["fresh_producer_intervals_observed"], 32)
        self.assertEqual(result["overlapping_delegated_parent_sequences"], [5])

    def test_sparse_campaign_and_missing_kind_cannot_qualify(self):
        raw, drag, inputs = campaign_fixture()
        raw["submissions"] = raw["submissions"][:2]
        raw["submissions_observed"] = 2
        self.assertTrue(owner.analyse(raw)["accepted"])
        self.assertFalse(owner.campaign_decision(raw, drag, inputs)["accepted"])
        for kind in ("windows_parent", "window_event", "completion_drain", "redraw", "loop_progress"):
            raw, drag, inputs = campaign_fixture()
            sample = next(item for item in raw["callbacks"] if item["kind"] == kind)
            sample["kind"] = "other"
            if sample["delegated_drag"] is not None:
                sample["delegated_drag"] = None
                sample["authored_ticks"] = sample["end_ticks"] - sample["begin_ticks"]
            self.assertFalse(owner.campaign_decision(raw, drag, inputs)["accepted"])

    def test_missing_drag_delegation_and_outside_observation_cannot_qualify(self):
        raw, drag, inputs = campaign_fixture()
        raw["callbacks"][-1]["delegated_drag"] = None
        raw["callbacks"][-1]["authored_ticks"] = 999800
        self.assertFalse(owner.campaign_decision(raw, drag, inputs)["accepted"])
        raw, drag, inputs = campaign_fixture()
        raw["callbacks"][-1]["delegated_drag"].update(begin_ticks=50000, end_ticks=999950)
        raw["callbacks"][-1]["authored_ticks"] = 49850
        self.assertFalse(owner.campaign_decision(raw, drag, inputs)["accepted"])

    def test_incomplete_physical_feedback_and_invalid_raw_span_reject(self):
        for mutation in (lambda drag, inputs: drag["samples"].pop(),
                         lambda drag, inputs: inputs["samples"][0].update(accepted=False),
                         lambda drag, inputs: drag["samples"][0].update(owned_move_loop_after=False),
                         lambda drag, inputs: drag["samples"][0].update(begin_ticks=True),
                         lambda drag, inputs: drag["samples"][1].update(begin_ticks=10000)):
            raw, drag, inputs = campaign_fixture()
            mutation(drag, inputs)
            self.assertRaises(ValueError, owner.campaign_decision, raw, drag, inputs)

    def test_performance_command_has_no_source_audit(self):
        command = owner.performance_command(Path("owned.exe"), Path("terminal.json"))
        self.assertIn("--timing-audit", command)
        self.assertIn("--receipt", command)
        self.assertNotIn("--frame-audit", command)
        self.assertNotIn("--reduced-motion", command)

    def test32_physical_input_retains_full_sampling_upper_bound(self):
        clock = Clock()
        capture = Capture(clock)
        pointer, records = Pointer(capture), []
        result = owner.input_samples(pointer, capture, clock, 32, records, Disclosure(capture))
        self.assertTrue(result["p95_below_50ms"])
        self.assertEqual(result["input_to_visible"]["count"], 32)
        self.assertEqual(pointer.events, ["click"] * 32)
        self.assertTrue(all(item["input_injection"]["inserted_events"] == 2 for item in records))
        for item in records:
            final = item["observations"][-1]
            self.assertEqual(item["input_to_visible_ms"], (final["end_ticks"] - item["input_begin_ticks"]) / 1000)
            self.assertGreater(item["input_to_visible_ms"], item["observer_sample_ms"])

    def test_button_pre_post_checks_stay_inside_timed_visible_interval(self):
        clock = Clock()
        capture = Capture(clock)
        pointer = Pointer(capture)
        click = pointer.click
        def checked_click():
            clock.ticks += 100000
            return click()
        pointer.click = checked_click
        records = []
        result = owner.input_samples(pointer, capture, clock, 32, records, Disclosure(capture))
        self.assertFalse(result["p95_below_50ms"])
        self.assertTrue(all(item["input_to_visible_ms"] > 100 for item in records))
        self.assertTrue(all(item["input_dispatch_end_ticks"] - item["input_begin_ticks"] > 100000 for item in records))

    def test_missing_insertion_provenance_and_click_failure_never_reach_roi_acceptance(self):
        for receipt in (dict(inserted_events=0, event_flags=[2, 4], prechecked=True, postchecked=True),
                        dict(inserted_events=2.0, event_flags=[2, 4], prechecked=True, postchecked=True),
                        dict(inserted_events=2, event_flags=[0xE003, 0xE005], prechecked=True, postchecked=True),
                        dict(inserted_events=2, event_flags=[2, 4], prechecked=True, postchecked=False)):
            clock = Clock()
            capture = Capture(clock)
            pointer = Pointer(capture)
            pointer.click = lambda receipt=receipt: receipt
            records = []
            self.assertRaises(RuntimeError, owner.input_samples, pointer, capture, clock, 32, records, Disclosure(capture))
            self.assertFalse(records[0]["accepted"])
            self.assertEqual(records[0]["observations"], [])
        clock = Clock()
        capture = Capture(clock)
        pointer = Pointer(capture)
        def failed_click():
            pointer.last_click = dict(inserted_events=1, event_flags=[2, 4], prechecked=True, postchecked=False)
            raise RuntimeError("partial batch insertion")
        pointer.click = failed_click
        records = []
        with self.assertRaisesRegex(RuntimeError, "partial batch insertion"):
            owner.input_samples(pointer, capture, clock, 32, records, Disclosure(capture))
        self.assertEqual(records[0]["input_injection"]["inserted_events"], 1)
        self.assertFalse(records[0]["accepted"])

    def test_observer_cost_not_subtracted_to_pass_budget(self):
        clock = Clock()
        capture = Capture(clock, cost=60000)
        result = owner.input_samples(Pointer(capture), capture, clock, 32, [], Disclosure(capture))
        self.assertFalse(result["p95_below_50ms"])
        self.assertGreater(result["input_to_visible"]["p95_ms"], 60)

    def test_old_label_never_copies_display_and_wait_scheduling_is_measured(self):
        clock, records = Clock(), []
        capture = Capture(clock)
        disclosure = Disclosure(capture)
        disclosure.pending = 2
        pointer = Pointer(capture)
        first_click = pointer.click
        def click():
            disclosure.pending = 2
            return first_click()
        pointer.click = click
        real_read = disclosure.read
        calls_at_reads = []
        def read(deadline):
            calls_at_reads.append(capture.calls)
            return real_read(deadline)
        disclosure.read = read
        def condition_wait(seconds):
            clock.ticks += round(seconds * clock.frequency)
        with patch.object(owner.time, "sleep", side_effect=condition_wait):
            result = owner.input_samples(pointer, capture, clock, 32, records, disclosure)
        self.assertTrue(result["p95_below_50ms"])
        for index, item in enumerate(records):
            self.assertEqual(len(item["label_observations"]), 3)
            self.assertEqual(len(item["observations"]), 1)
            self.assertEqual(calls_at_reads[index * 4:index * 4 + 4], [index * 3 + 2] * 4)
            self.assertGreaterEqual(item["label_ready_end_ticks"] - item["input_dispatch_end_ticks"], 4000)
            self.assertGreaterEqual(item["observations"][0]["begin_ticks"], item["label_ready_end_ticks"])
            self.assertEqual(item["input_to_visible_ms"],
                             (item["observed_end_ticks"] - item["input_begin_ticks"]) / 1000)

    def test_never_ready_late_and_foreign_label_cannot_reach_screen_copy(self):
        for mode in ("never", "late", "foreign", "wrong_frequency", "reversed", "unexpected"):
            with self.subTest(mode=mode):
                clock, records = Clock(), []
                capture = Capture(clock)
                disclosure = Disclosure(capture)
                read = disclosure.read
                def observed(deadline):
                    sample = read(deadline)
                    if capture.pressed:
                        if mode == "never":
                            sample["label"] = owner.LABELS[0]
                        elif mode == "late":
                            sample["end_ticks"] = deadline
                        elif mode == "foreign":
                            raise owner.DisclosureError("verify_disclosure", "Foreign BUTTON")
                        elif mode == "wrong_frequency":
                            sample["qpc_frequency"] += 1
                        elif mode == "reversed":
                            sample["begin_ticks"] = sample["end_ticks"] + 1
                        elif mode == "unexpected":
                            sample["label"] = "Unexpected action"
                    return sample
                disclosure.read = observed
                with patch.object(owner.time, "sleep"):
                    self.assertRaises(owner.DisclosureError, owner.input_samples,
                                      Pointer(capture), capture, clock, 32, records, disclosure)
                self.assertEqual(capture.calls, 2)
                self.assertFalse(records[0]["accepted"])
                self.assertEqual(records[0]["observations"], [])
                if mode == "never":
                    self.assertEqual(len(records[0]["label_observations"]), 256)

    def test_label_only_never_accepts_missing_or_wrong_display_pixels(self):
        clock, records = Clock(), []
        capture = Capture(clock)
        capture.stuck = True
        self.assertRaises(RuntimeError, owner.input_samples, Pointer(capture), capture,
                          clock, 32, records, Disclosure(capture))
        self.assertEqual(records[0]["label_observations"][-1]["label"], owner.LABELS[1])
        self.assertFalse(records[0]["accepted"])
        self.assertEqual(len(records[0]["observations"]), 256)

    def test_label_wait_cost_is_not_subtracted_and_deadline_precedes_matching_state(self):
        clock, records = Clock(), []
        capture, disclosure = Capture(clock), None
        disclosure = Disclosure(capture)
        actual = disclosure.read
        def read(deadline):
            if capture.pressed:
                clock.ticks += 60000
            return actual(deadline)
        disclosure.read = read
        result = owner.input_samples(Pointer(capture), capture, clock, 32, records, disclosure)
        self.assertFalse(result["p95_below_50ms"])
        self.assertTrue(all(item["input_to_visible_ms"] > 60 for item in records))
        self.assertTrue(all(item["observations"][0]["begin_ticks"] >= item["label_ready_end_ticks"] for item in records))

    def test_label_baseline_cannot_change_without_measured_click_and_failed_read_retains_cause(self):
        clock, records = Clock(), []
        capture, disclosure = Capture(clock), None
        disclosure = Disclosure(capture)
        read = disclosure.read
        def observed(deadline):
            sample = read(deadline)
            if disclosure.calls == 3:
                sample["label"] = owner.LABELS[0]
            return sample
        disclosure.read = observed
        self.assertRaises(owner.DisclosureError, owner.input_samples, Pointer(capture), capture,
                          clock, 32, records, disclosure)
        self.assertEqual(len(records), 2)
        self.assertTrue(records[0]["accepted"])
        self.assertFalse(records[1]["accepted"])
        clock, records = Clock(), []
        capture = Capture(clock)
        disclosure = Disclosure(capture)
        read = disclosure.read
        def failed(deadline):
            if capture.pressed:
                disclosure.last_read = dict(accepted=False, failure=dict(message="original WM_GETTEXT failure"))
                raise owner.DisclosureError("read_disclosure_text", "original WM_GETTEXT failure")
            return read(deadline)
        disclosure.read = failed
        self.assertRaises(owner.DisclosureError, owner.input_samples, Pointer(capture), capture,
                          clock, 32, records, disclosure)
        self.assertIn("original WM_GETTEXT failure", records[0]["label_observations"][0]["failure"]["message"])
        self.assertFalse(records[0]["accepted"])

    def test_late_changed_frame_foreign_roi_missing_feedback_and_clock_reject(self):
        for field in ("late", "foreign", "stuck", "wrong_frequency"):
            with self.subTest(field=field):
                clock = Clock()
                capture = Capture(clock)
                setattr(capture, field, True)
                records = []
                with self.assertRaises(RuntimeError):
                    owner.input_samples(Pointer(capture), capture, clock, 32, records, Disclosure(capture))
                self.assertTrue(all(not record["accepted"] for record in records))

    def test_alpha_drift_alone_and_one_pixel_changes_never_prove_disclosure(self):
        baseline = bytes([1, 2, 3, 0]) * 4
        self.assertFalse(owner.pixels_changed(baseline, bytes([1, 2, 3, 255]) * 4))
        self.assertFalse(owner.pixels_changed(baseline, bytes([100, 100, 100, 0]) + baseline[4:]))

    def test32_physical_drag_checks_owned_loop_and_strict_positions(self):
        clock, observer = Clock(), Observer()
        pointer = Pointer(None)
        actual_move = pointer.move_screen
        def move(target):
            actual_move(target)
            observer.current = target
        pointer.move_screen = move
        records = []
        with patch.object(owner.time, "perf_counter", return_value=0), patch.object(owner.time, "sleep"):
            # Avoid waiting on the cadence: successive clock reads are already after every target.
            with patch.object(owner.time, "perf_counter", side_effect=[0] + [3] * 1000):
                result = owner.drag_path(observer, pointer, 11, 55, (0, 0), (0, 0), (1, 1), 10, 32, clock, records)
        self.assertEqual(len(result["samples"]), 32)
        self.assertEqual(observer.calls, 64)
        self.assertEqual(records[-1]["actual_origin"], (128, 64))

    def test_drag_foreign_loop_before_after_and_late_position_reject(self):
        for mode in ("before", "after", "late_position", "wrong_cursor"):
            observer, pointer, clock, records = Observer(), Pointer(None), Clock(), []
            pointer.point = (0, 0)
            def move(target):
                pointer.point = (999, 999) if mode == "wrong_cursor" else target
                observer.current = target
            pointer.move_screen = move
            observer.active = mode != "before"
            observer.foreign_after = mode == "after"
            observer.wrong_position = mode == "late_position"
            times = iter([0, 3, 3, 3, 10, 10]) if mode == "late_position" else iter([0] + [3] * 100)
            with patch.object(owner.time, "perf_counter", side_effect=lambda: next(times)), patch.object(owner.time, "sleep"):
                with self.assertRaises(RuntimeError):
                    owner.drag_path(observer, pointer, 11, 55, (0, 0), (0, 0), (1, 1), 10, 32, clock, records)
            self.assertFalse(records[0]["accepted"])

    def test_wrong_sample_inventory_is_rejected(self):
        self.assertRaises(ValueError, owner.input_samples, None, None, None, 31, [], None)
        self.assertRaises(ValueError, owner.drag_path, None, None, 11, 55, None, None, None, 0, 31, None, [])

    def test_dxgi_pending_retains_original_floor_deadline_inventory_and_full_cost(self):
        clock, records, options = Clock(), [], []
        capture = Capture(clock)
        original = capture.sample
        pending_after_click = [0]
        def sample(**kwargs):
            options.append(dict(kwargs))
            observed = original(**kwargs)
            if not capture.pressed:
                pending = capture.calls == 1
            else:
                pending_after_click[0] += 1
                pending = pending_after_click[0] % 3 != 0
            return dxgi_observation(observed, pending=pending)
        capture.sample = sample
        result = owner.input_samples(Pointer(capture), capture, clock, 32, records, Disclosure(capture))
        self.assertTrue(result["p95_below_50ms"])
        self.assertEqual(records[0]["baseline_observations"][0]["kind"], "pending")
        for item in records:
            post = item["observations"]
            self.assertTrue(any(attempt["kind"] == "pending" for attempt in post))
            start = item["input_begin_ticks"]
            self.assertEqual(item["input_to_visible_ms"], (post[-1]["end_ticks"] - start) / 1000)
        post_options = [item for item in options if item["minimum_present_ticks"]]
        for item in post_options:
            self.assertEqual(item["deadline_ticks"], item["minimum_present_ticks"] + clock.frequency)

    def test_never_ready_baseline_uses256_total_attempts_without_a_click_or_new_deadline(self):
        clock, records, bounds = Clock(), [], []
        capture = Capture(clock)
        original = capture.sample
        def sample(**kwargs):
            bounds.append(kwargs["deadline_ticks"])
            return dxgi_observation(original(**kwargs), pending=True)
        capture.sample = sample
        pointer = Pointer(capture)
        self.assertRaises(RuntimeError, owner.input_samples, pointer, capture, clock, 32, records, Disclosure(capture))
        self.assertEqual(len(bounds), 256)
        self.assertEqual(len(set(bounds)), 1)
        self.assertEqual(pointer.events, [])
        self.assertEqual(len(records[0]["baseline_observations"]), 256)
        self.assertFalse(records[0]["accepted"])

    def test_zero_future_and_preinput_source_cannot_qualify_even_matching_final_pixels(self):
        for mode in ("zero", "stale", "future"):
            clock, records = Clock(), []
            capture = Capture(clock)
            original = capture.sample
            def sample(**kwargs):
                observed = original(**kwargs)
                floor = kwargs["minimum_present_ticks"]
                source = observed.begin_ticks
                if floor:
                    source = 0 if mode == "zero" else floor - 1 if mode == "stale" else observed.end_ticks + 1
                return dxgi_observation(observed, source=source)
            capture.sample = sample
            self.assertRaises(RuntimeError, owner.input_samples, Pointer(capture), capture, clock, 32, records, Disclosure(capture))
            self.assertFalse(records[0]["accepted"])

    def test_helper_pair_invalid_hash_and_loader_failure_are_before_product_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory) / "helper.dll"
            fixture.write_bytes(b"test fixture")
            for helper, digest in ((None, "0" * 64), (fixture, None), (fixture, "0" * 64), (Path("relative.dll"), "0" * 64)):
                args = SimpleNamespace(capture_helper=helper, capture_helper_sha256=digest, timing_profile="strict")
                with patch.object(owner.subprocess, "Popen") as launch:
                    self.assertRaises(ValueError, owner.run, args)
                launch.assert_not_called()
            digest = owner.hashlib.sha256(fixture.read_bytes()).hexdigest()
            args = SimpleNamespace(capture_helper=fixture, capture_helper_sha256=digest, timing_profile="strict")
            with patch.object(owner, "DxgiLibrary", side_effect=AttributeError("Missing ABI export")), patch.object(owner.subprocess, "Popen") as launch:
                self.assertRaises(AttributeError, owner.run, args)
            launch.assert_not_called()

    def failed_run(self, directory, *, collect_failure=False, foreign=False, bad_identity=False, profile="strict"):
        executable = Path(directory) / "fixture.exe"
        executable.write_bytes(b"fixture")
        process = SimpleNamespace(pid=55, _handle=10, returncode=None, poll=lambda: process.returncode)
        events = []
        class Output:
            complete = False
            def __init__(self, *_):
                pass
            def collect(self, phase):
                events.append(phase)
                if collect_failure:
                    raise RuntimeError("cleanup drain cause")
                self.complete, process.returncode = True, 0
            def failed_teardown(self):
                events.append("forced")
                self.complete, process.returncode = True, -1
            def receipt(self):
                return dict(events=events)
        def window_thread(hwnd, identity):
            identity._obj.value = 55
            return 71
        native = SimpleNamespace(user=SimpleNamespace(GetWindowThreadProcessId=window_thread), find_window=lambda *_: 11, post=lambda *_: events.append("WM_CLOSE"),
                                 owns=lambda *_: not foreign)
        processes = SimpleNamespace(identity=lambda *_: (56 if bad_identity else 55, executable.resolve(), 99),
                                    alive=lambda *_: True)
        args = SimpleNamespace(executable=executable, output=Path(directory) / "out", workers=0, affinity_cpus=None, timing_profile=profile)
        with ExitStack() as stack:
            for name, value in (("NativeWindows", lambda: native), ("ReferenceProcesses", lambda: processes),
                                ("Qpc", Clock), ("ObservationWindows", lambda _: None), ("OperationalOutput", Output),
                                ("observe_exposure", lambda *_: None), ("physical_drag", lambda *_: (_ for _ in ()).throw(RuntimeError("primary movement cause")))):
                stack.enter_context(patch.object(owner, name, value))
            stack.enter_context(patch.object(owner.subprocess, "Popen", return_value=process))
            try:
                owner.run(args)
            except BaseException as error:
                return events, error, owner.json.loads((args.output / "observation.json").read_text(encoding="utf-8"))
        self.fail("Failed scenario qualified unexpectedly")

    def test_failure_gracefully_closes_owned_process_before_forced_cleanup(self):
        with tempfile.TemporaryDirectory() as directory:
            events, error, receipt = self.failed_run(directory)
        self.assertEqual(events, ["WM_CLOSE", "failed_scenario_graceful_close"])
        self.assertIn("primary movement cause", str(error))
        self.assertFalse(receipt["qualification_accepted"])

    def test_primary_and_cleanup_failure_preserved_and_foreign_window_not_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            events, error, receipt = self.failed_run(directory, collect_failure=True)
        self.assertIsInstance(error, BaseExceptionGroup)
        self.assertIn("forced", events)
        self.assertEqual(len(receipt["failure_causes"]), 2)
        with tempfile.TemporaryDirectory() as directory:
            events, _, _ = self.failed_run(directory, foreign=True)
        self.assertEqual(events, ["forced"])

    def test_native_process_wrong_identity_never_reaches_input(self):
        with tempfile.TemporaryDirectory() as directory:
            events, error, receipt = self.failed_run(directory, bad_identity=True)
        self.assertIn("identity differs", str(error))
        self.assertNotIn("WM_CLOSE", events)
        self.assertNotIn("drag_state", receipt)


if __name__ == "__main__":
    unittest.main()
