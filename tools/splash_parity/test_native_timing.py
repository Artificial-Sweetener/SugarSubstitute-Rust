# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Invalid native wire fixtures and observable timing analysis tests."""
import copy
import json
import tempfile
import unittest
import io
import sys
from pathlib import Path
from unittest.mock import patch

try:
    from .native_timing import MAX_FILE_BYTES, MAX_SAMPLES, STARTUP_STAGES, analyse, read_terminal, validate
except ImportError:
    from native_timing import MAX_FILE_BYTES, MAX_SAMPLES, STARTUP_STAGES, analyse, read_terminal, validate


def fixture():
    return {"schema_version": 1, "source": "windows_qpc", "begin_ticks": 100, "end_ticks": 100000,
            "frequency": 1000000, "ui_thread_id": 71, "callbacks_observed": 2, "callback_samples_omitted": 0,
            "submissions_observed": 3, "submission_samples_omitted": 0,
            "callbacks": [dict(sequence=1, kind="completion_drain", begin_ticks=210, end_ticks=300,
                               authored_ticks=90, delegated_drag=None),
                          dict(sequence=2, kind="windows_parent", begin_ticks=200, end_ticks=400,
                               authored_ticks=100, delegated_drag=dict(begin_ticks=250, end_ticks=350))],
            "submissions": [dict(sequence=i, begin_ticks=1000 + i * 16000, end_ticks=1100 + i * 16000,
                                 retained=False, identity=dict(surface=1, state_generation=1, sequence=i))
                            for i in range(1, 4)]}


def startup_fixture():
    value = fixture()
    value.update(schema_version=2, startup_milestones=[dict(stage=stage, ticks=120 + index * 10)
                                                    for index, stage in enumerate(STARTUP_STAGES)])
    return value


class StartupTimingTests(unittest.TestCase):
    def test_current_and_preserved_receipts_keep_exact_observed_startup_ticks(self):
        result = analyse(startup_fixture())
        self.assertEqual(result["native_receipt_schema_version"], 2)
        self.assertEqual(result["startup_milestones"][-1]["ticks"], 180)
        self.assertEqual(result["startup_milestones"][-1]["elapsed_ms"], .08)
        self.assertEqual(analyse(fixture())["startup_milestones"], [])

    def test_partial_prefix_is_diagnostic_only_without_successful_submissions(self):
        value = startup_fixture()
        value.update(submissions=[], submissions_observed=0)
        value["startup_milestones"] = value["startup_milestones"][:3]
        self.assertIs(validate(value), value)
        self.assertFalse(analyse(value)["accepted"])
        value["submissions"] = fixture()["submissions"]
        value["submissions_observed"] = 3
        self.assertRaises(ValueError, validate, value)

    def test_startup_schema_inventory_and_stage_order_reject_corruption(self):
        mutations = [lambda value: value.pop("startup_milestones"),
                     lambda value: value.__setitem__("startup_milestones", {}),
                     lambda value: value["startup_milestones"].append(dict(stage="future", ticks=190)),
                     lambda value: value["startup_milestones"][1].__setitem__("stage", "event_loop_ready"),
                     lambda value: value["startup_milestones"][0].__setitem__("stage", []),
                     lambda value: value["startup_milestones"][0].__setitem__("future", 1),
                     lambda value: value.__setitem__("schema_version", 3)]
        for mutate in mutations:
            value = startup_fixture()
            mutate(value)
            with self.subTest(value=value), self.assertRaises(ValueError):
                validate(value)
        legacy = fixture()
        legacy["startup_milestones"] = []
        self.assertRaises(ValueError, validate, legacy)

    def test_startup_clock_types_lifetime_and_submission_order_are_strict(self):
        for index, ticks in ((0, True), (0, 120.0), (0, 99), (1, 119), (0, 1 << 63),
                             (6, 100001), (6, 17001)):
            value = startup_fixture()
            value["startup_milestones"][index]["ticks"] = ticks
            with self.subTest(index=index, ticks=ticks), self.assertRaises(ValueError):
                validate(value)

    def test_startup_ticks_above_float_precision_are_preserved(self):
        value = startup_fixture()
        offset = 1 << 53
        value["begin_ticks"] += offset
        value["end_ticks"] += offset
        for mark in value["startup_milestones"]:
            mark["ticks"] += offset
        for sample in value["callbacks"] + value["submissions"]:
            sample["begin_ticks"] += offset
            sample["end_ticks"] += offset
            if sample.get("delegated_drag"):
                sample["delegated_drag"]["begin_ticks"] += offset
                sample["delegated_drag"]["end_ticks"] += offset
        result = analyse(value)
        self.assertEqual(result["startup_milestones"][-1]["ticks"], offset + 180)
        self.assertEqual(result["startup_milestones"][-1]["elapsed_ms"], .08)


class NativeTimingTests(unittest.TestCase):
    def test_busy_declares_callback_and_producer_margins_and_keeps_strict_verdicts(self):
        value = fixture()
        value["callbacks"][0].update(begin_ticks=100, end_ticks=10100, authored_ticks=10000)
        value["callbacks"][1].update(begin_ticks=100, end_ticks=11100, authored_ticks=10900)
        value["submissions"][2].update(begin_ticks=63000, end_ticks=63100)
        preserved = copy.deepcopy(value)
        strict, busy = analyse(value), analyse(value, profile="busy")
        self.assertFalse(strict["accepted"])
        self.assertTrue(busy["accepted"])
        self.assertFalse(busy["callbacks_budget_passed"])
        self.assertFalse(busy["producer_p95_below_25ms"])
        self.assertFalse(busy["timing_assessment"]["strict_accepted"])
        self.assertEqual(busy["timing_assessment"]["producer"]["nominal_limit_ms"], 25)
        self.assertAlmostEqual(busy["timing_assessment"]["producer"]["margin_ms"], 1000 / 60)
        for kind in ("windows_parent", "completion_drain"):
            self.assertEqual(strict["callbacks_by_kind"][kind]["authored"], busy["callbacks_by_kind"][kind]["authored"])
            self.assertEqual(busy["callbacks_by_kind"][kind]["timing_assessment"]["p95"]["nominal_limit_ms"], 4)
        self.assertEqual(value, preserved)
        self.assertEqual(strict, analyse(value, profile="strict"))
        self.assertNotIn("timing_assessment", strict)

    def test_busy_cannot_accept_empty_missing_or_omitted_native_evidence(self):
        for buffer, count in (("callbacks", "callbacks_observed"), ("submissions", "submissions_observed")):
            value = fixture()
            value[buffer], value[count] = [], 0
            self.assertFalse(analyse(value, profile="busy")["accepted"])
        value = fixture()
        value["callbacks"] = [dict(sequence=i, kind="other", begin_ticks=200, end_ticks=200,
                                   authored_ticks=0, delegated_drag=None) for i in range(1, MAX_SAMPLES + 1)]
        value.update(callbacks_observed=MAX_SAMPLES + 1, callback_samples_omitted=1)
        self.assertFalse(analyse(value, profile="busy")["accepted"])
        for change in (dict(frequency=float("inf")), dict(callbacks=None), dict(end_ticks=float("nan"))):
            self.assertRaises(ValueError, analyse, dict(fixture(), **change), profile="busy")
        self.assertRaises(ValueError, analyse, None, profile="unknown")

    def test_all_timing_clis_reject_unknown_profile_before_filesystem_or_native_effects(self):
        from tools.splash_parity import native_timing, native_windows, startup_windows, m0_windows
        cases = [(native_timing, ["missing.json", "--output", "out.json"], "read_terminal"),
                 (native_windows, ["--executable", "missing.exe", "--output", "out"], "NativeWindows"),
                 (m0_windows, ["--executable", "missing.exe", "--output", "out"], "NativeWindows"),
                 (startup_windows, ["--mode", "rust", "--executable", "missing.exe", "--output", "out", "--temporary", "tmp"], "NativeWindows")]
        for module, arguments, boundary in cases:
            with self.subTest(module=module.__name__), patch.object(sys, "argv", ["fixture", *arguments, "--timing-profile", "unknown"]), \
                    patch.object(sys, "stderr", io.StringIO()), patch.object(module, boundary) as external, \
                    patch.object(Path, "mkdir") as mkdir, patch.object(Path, "open") as read:
                with self.assertRaises(SystemExit) as caught:
                    module.main()
                self.assertEqual(caught.exception.code, 2)
                external.assert_not_called()
                mkdir.assert_not_called()
                read.assert_not_called()

    def reject(self, mutation):
        value = fixture()
        mutation(value)
        with self.assertRaises(ValueError):
            validate(value)

    def test_nested_completion_starts_reverse_without_summing(self):
        result = analyse(fixture())
        self.assertTrue(result["accepted"])
        self.assertEqual(result["callbacks_by_kind"]["windows_parent"]["full"]["p95_ms"], .2)
        self.assertEqual(result["callbacks_by_kind"]["windows_parent"]["authored"]["p95_ms"], .1)
        self.assertEqual(result["fresh_producer_submission_intervals"]["count"], 2)

    def test_wire_types_schema_enums_and_positive_identities(self):
        for field, invalid in (("schema_version", True), ("begin_ticks", 100.0), ("end_ticks", float("inf")),
                               ("frequency", 0), ("frequency", 1 << 63), ("ui_thread_id", False), ("source", "monotonic")):
            with self.subTest(field=field):
                self.reject(lambda value: value.__setitem__(field, invalid))
        self.reject(lambda value: value.__setitem__("future_field", 1))
        self.reject(lambda value: value["callbacks"][0].__setitem__("kind", "idle"))
        for field in ("surface", "state_generation", "sequence"):
            self.reject(lambda value, field=field: value["submissions"][0]["identity"].__setitem__(field, 0))

    def test_raw_ticks_outside_lifetime_reversed_completion_and_overlap(self):
        self.reject(lambda value: value["callbacks"][0].__setitem__("begin_ticks", 99))
        self.reject(lambda value: value["callbacks"][0].__setitem__("end_ticks", 401))
        self.reject(lambda value: value["submissions"][1].__setitem__("begin_ticks", 17000))
        self.reject(lambda value: value["submissions"][2].__setitem__("end_ticks", 100001))
        self.reject(lambda value: value["callbacks"][1].__setitem__("sequence", 3))
        self.reject(lambda value: value["submissions"][1].__setitem__("sequence", 3))

    def test_impossible_crossing_callbacks_and_untyped_kind_reject(self):
        self.reject(lambda value: value["callbacks"][1].update(begin_ticks=250, authored_ticks=50))
        self.reject(lambda value: value["callbacks"][0].__setitem__("kind", []))

    def test_delegated_interval_exact_and_only_parent(self):
        self.reject(lambda value: value["callbacks"][1].__setitem__("authored_ticks", 99))
        self.reject(lambda value: value["callbacks"][1]["delegated_drag"].__setitem__("end_ticks", 401))
        self.reject(lambda value: value["callbacks"][1].__setitem__("kind", "redraw"))
        self.reject(lambda value: value["callbacks"][0].__setitem__("authored_ticks", True))

    def test_retained_exposure_not_producer_and_flag_must_match(self):
        value = fixture()
        value["submissions"][1]["identity"] = copy.deepcopy(value["submissions"][0]["identity"])
        self.assertRaises(ValueError, validate, value)
        value["submissions"][1]["retained"] = True
        result = analyse(value)
        self.assertEqual(result["retained_exposures"], 1)
        self.assertEqual(result["fresh_producer_submission_intervals"]["p95_ms"], 32)
        self.assertEqual(result["all_lifetime_submission_intervals"]["p95_ms"], 16)
        self.assertFalse(result["accepted"])
        self.reject(lambda value: value["submissions"][0].__setitem__("retained", True))

    def test_strict_callback_and_producer_budgets(self):
        value = fixture()
        value["callbacks"][0].update(begin_ticks=100, end_ticks=4100, authored_ticks=4000)
        value["callbacks"][1].update(begin_ticks=100, end_ticks=5100, authored_ticks=4900)
        self.assertFalse(analyse(value)["callbacks_budget_passed"])
        value = fixture()
        value["submissions"][2].update(begin_ticks=59000, end_ticks=59100)
        self.assertEqual(analyse(value)["fresh_producer_submission_intervals"]["p95_ms"], 26)
        self.assertFalse(analyse(value)["producer_p95_below_25ms"])

    def test_capacity_omissions_and_incomplete_evidence_rejected_for_acceptance(self):
        self.reject(lambda value: value.__setitem__("callbacks_observed", 3))
        self.reject(lambda value: value.__setitem__("callback_samples_omitted", 1))
        value = fixture()
        value["callbacks"] = [dict(sequence=i, kind="other", begin_ticks=200, end_ticks=200,
                                   authored_ticks=0, delegated_drag=None) for i in range(1, MAX_SAMPLES + 1)]
        value.update(callbacks_observed=MAX_SAMPLES + 1, callback_samples_omitted=1)
        self.assertFalse(analyse(value)["accepted"])
        value["callbacks"].append(value["callbacks"][-1])
        self.assertRaises(ValueError, validate, value)

    def test_bounded_terminal_duplicate_keys_and_host_agreement(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "terminal.json"
            host = {"frames_presented": 3, "native_timings": fixture()}
            path.write_text(json.dumps(host), encoding="utf-8")
            self.assertEqual(read_terminal(path)["frequency"], 1000000)
            host["frames_presented"] = 4
            path.write_text(json.dumps(host), encoding="utf-8")
            self.assertRaises(ValueError, read_terminal, path)
            path.write_text('{"native_timings":{},"native_timings":{}}', encoding="utf-8")
            self.assertRaises(ValueError, read_terminal, path)
            with path.open("wb") as stream:
                stream.truncate(MAX_FILE_BYTES + 1)
            self.assertRaises(ValueError, read_terminal, path)

    def test_integer_precision_preserved_above_javascript_exact_limit(self):
        value = fixture()
        shift = 9007199254740993
        value["begin_ticks"] += shift
        value["end_ticks"] += shift
        for sample in value["callbacks"] + value["submissions"]:
            sample["begin_ticks"] += shift
            sample["end_ticks"] += shift
            if sample.get("delegated_drag"):
                sample["delegated_drag"]["begin_ticks"] += shift
                sample["delegated_drag"]["end_ticks"] += shift
        self.assertEqual(analyse(value)["fresh_producer_submission_intervals"]["p95_ms"], 16)


if __name__ == "__main__":
    unittest.main()
