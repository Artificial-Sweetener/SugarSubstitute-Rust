# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Invalid native wire fixtures and observable timing analysis tests."""
import copy
import json
import tempfile
import unittest
from pathlib import Path

try:
    from .native_timing import MAX_FILE_BYTES, MAX_SAMPLES, analyse, read_terminal, validate
except ImportError:
    from native_timing import MAX_FILE_BYTES, MAX_SAMPLES, analyse, read_terminal, validate


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


class NativeTimingTests(unittest.TestCase):
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
