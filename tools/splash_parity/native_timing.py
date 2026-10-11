# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Validate bounded raw native timing evidence without summing nested callbacks."""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

MAX_SAMPLES = 16384
MAX_FILE_BYTES = 8 * 1024 * 1024
KINDS = frozenset(("windows_parent", "windows_child", "window_event", "completion_drain",
                   "native_action", "redraw", "loop_progress", "other"))
RECEIPT_KEYS = frozenset(("schema_version", "source", "begin_ticks", "end_ticks", "frequency",
                        "ui_thread_id", "callbacks_observed", "callback_samples_omitted",
                        "submissions_observed", "submission_samples_omitted", "callbacks", "submissions"))


def integer(value, name, minimum=0, maximum=(1 << 64) - 1):
    """Reject bool, rounded JSON floats and integers outside the native wire type."""
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"{name} requires an integer in [{minimum}, {maximum}]")
    return value


def keys(value, expected, name):
    if not isinstance(value, dict) or set(value) != set(expected):
        raise ValueError(f"{name} schema is missing or unrecognized")


def interval(value, lifetime, name):
    begin = integer(value["begin_ticks"], f"{name} begin", 0, (1 << 63) - 1)
    end = integer(value["end_ticks"], f"{name} end", begin, (1 << 63) - 1)
    if begin < lifetime[0] or end > lifetime[1]:
        raise ValueError(f"{name} lies outside its observed lifetime")
    return begin, end


def validate(receipt):
    """Validate raw clock, complete count accounting, nesting and successful identities.

    Omissions remain valid observable metadata, but analyse never accepts an
    incomplete buffer. Callback sequence is completion order; starts may nest.
    """
    keys(receipt, RECEIPT_KEYS, "native timings")
    if integer(receipt["schema_version"], "schema version", 1, 1) != 1 or receipt["source"] != "windows_qpc":
        raise ValueError("Unsupported native clock schema")
    integer(receipt["frequency"], "QPC frequency", 1, (1 << 63) - 1)
    integer(receipt["ui_thread_id"], "UI thread identity", 1, (1 << 32) - 1)
    lifetime = interval(receipt, (0, (1 << 63) - 1), "native lifetime")
    if lifetime[0] == lifetime[1]:
        raise ValueError("Native timing lifetime must be positive")
    for buffer, count, omitted in (("callbacks", "callbacks_observed", "callback_samples_omitted"),
                                   ("submissions", "submissions_observed", "submission_samples_omitted")):
        samples = receipt[buffer]
        if not isinstance(samples, list) or len(samples) > MAX_SAMPLES:
            raise ValueError(f"{buffer} exceeds bounded native inventory")
        observed = integer(receipt[count], count)
        missing = integer(receipt[omitted], omitted)
        if observed != len(samples) + missing or missing and len(samples) != MAX_SAMPLES:
            raise ValueError(f"{buffer} observed/omitted accounting differs from native capacity")
    previous_end = lifetime[0]
    for sequence, sample in enumerate(receipt["callbacks"], 1):
        keys(sample, ("sequence", "kind", "begin_ticks", "end_ticks", "authored_ticks", "delegated_drag"), "callback")
        if (integer(sample["sequence"], "callback sequence", 1) != sequence
                or not isinstance(sample["kind"], str) or sample["kind"] not in KINDS):
            raise ValueError("Unknown callback kind or nonconsecutive completion sequence")
        begin, end = interval(sample, lifetime, "callback")
        if end < previous_end:
            raise ValueError("Callback completion times reversed")
        previous_end = end
        delegated = sample["delegated_drag"]
        delegated_ticks = 0
        if delegated is not None:
            keys(delegated, ("begin_ticks", "end_ticks"), "delegated drag")
            if sample["kind"] != "windows_parent":
                raise ValueError("Delegated system move belongs only to the parent subclass")
            drag_begin, drag_end = interval(delegated, (begin, end), "delegated drag")
            delegated_ticks = drag_end - drag_begin
        if integer(sample["authored_ticks"], "authored ticks") != end - begin - delegated_ticks:
            raise ValueError("Authored ticks differ from exact full callback minus delegated move")
    stack = []
    for begin, end in sorted(((sample["begin_ticks"], sample["end_ticks"]) for sample in receipt["callbacks"]),
                             key=lambda pair: (pair[0], -pair[1])):
        while stack and begin >= stack[-1]:
            stack.pop()
        if stack and end > stack[-1]:
            raise ValueError("Single UI thread callback intervals cross instead of nesting")
        stack.append(end)
    previous_end, previous_identity = lifetime[0], None
    for sequence, sample in enumerate(receipt["submissions"], 1):
        keys(sample, ("sequence", "begin_ticks", "end_ticks", "retained", "identity"), "submission")
        if integer(sample["sequence"], "submission sequence", 1) != sequence:
            raise ValueError("Successful submission sequence is not consecutive")
        begin, end = interval(sample, lifetime, "submission")
        if begin < previous_end:
            raise ValueError("Sequential UI submissions overlap or reverse")
        previous_end = end
        identity = sample["identity"]
        keys(identity, ("surface", "state_generation", "sequence"), "frame identity")
        for field in identity:
            integer(identity[field], f"frame {field}", 1)
        if type(sample["retained"]) is not bool or sample["retained"] != (identity == previous_identity):
            raise ValueError("Retained exposure differs from previous actual successful identity")
        previous_identity = identity
    return receipt


def distribution(values):
    """Use nearest-rank percentiles; preserve sample count and maxima without smoothing."""
    if not values:
        return {"count": 0, "p50_ms": None, "p95_ms": None, "p99_ms": None, "max_ms": None}
    ordered = sorted(values)
    if any(not math.isfinite(value) or value < 0 for value in ordered):
        raise ValueError("Distribution values must be finite nonnegative observed milliseconds")
    return {"count": len(ordered), **{f"p{percent}_ms": ordered[math.ceil(len(ordered) * percent / 100) - 1]
                                    for percent in (50, 95, 99)}, "max_ms": ordered[-1]}


def analyse(receipt):
    """Keep full/delegated/authored evidence and assess explicit strict native budgets."""
    validate(receipt)
    factor = 1000 / receipt["frequency"]
    complete = receipt["callback_samples_omitted"] == receipt["submission_samples_omitted"] == 0
    groups = {}
    callback_passes = []
    for kind in sorted(KINDS):
        samples = [sample for sample in receipt["callbacks"] if sample["kind"] == kind]
        full = distribution([(sample["end_ticks"] - sample["begin_ticks"]) * factor for sample in samples])
        authored = distribution([sample["authored_ticks"] * factor for sample in samples])
        passed = bool(samples) and authored["p95_ms"] < 4 and authored["p99_ms"] < 8
        groups[kind] = {"full": full, "authored": authored, "authored_p95_below_4ms_p99_below_8ms": passed if samples else None,
                        "delegated_drag": [{**sample["delegated_drag"], "sequence": sample["sequence"]}
                                           for sample in samples if sample["delegated_drag"] is not None]}
        if samples:
            callback_passes.append(passed)
    all_submissions = receipt["submissions"]
    fresh = [sample for sample in all_submissions if not sample["retained"]]
    intervals = lambda samples: distribution([(right["end_ticks"] - left["end_ticks"]) * factor
                                              for left, right in zip(samples, samples[1:])])
    fresh_intervals, lifetime_intervals = intervals(fresh), intervals(all_submissions)
    producer_passed = fresh_intervals["count"] > 0 and fresh_intervals["p95_ms"] < 25
    return {"schema_version": 1, "source": receipt["source"], "frequency": receipt["frequency"],
            "ui_thread_id": receipt["ui_thread_id"], "complete_evidence": complete,
            "callbacks_by_kind": groups, "fresh_producer_submission_intervals": fresh_intervals,
            "all_lifetime_submission_intervals": lifetime_intervals,
            "fresh_producer_submissions": len(fresh), "retained_exposures": len(all_submissions) - len(fresh),
            "callbacks_budget_passed": bool(callback_passes) and all(callback_passes),
            "producer_p95_below_25ms": producer_passed,
            "accepted": complete and bool(callback_passes) and all(callback_passes) and producer_passed,
            "acceptance_scope": "Observed native distributions; campaign completeness is separately assessed by the orchestrator",
            "scope": "Native successful submission end-to-end intervals; retained exposures excluded from producer cadence. Full nested callback distributions are never summed. Display latency is outside this receipt."}


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"Duplicate JSON key: {key}")
        value[key] = item
    return value


def read_terminal(path: Path):
    """Read bounded host terminal metadata with duplicate-key rejection."""
    with path.open("rb") as stream:
        raw = stream.read(MAX_FILE_BYTES + 1)
    if len(raw) > MAX_FILE_BYTES:
        raise ValueError("Native terminal receipt exceeds eight MiB")
    host = json.loads(raw, object_pairs_hook=unique_object)
    if not isinstance(host, dict) or "native_timings" not in host:
        raise ValueError("Host terminal native_timings is missing")
    receipt = validate(host["native_timings"])
    presented = integer(host.get("frames_presented"), "host submitted frame inventory")
    if presented != receipt["submissions_observed"]:
        raise ValueError("Host lifetime submissions disagree with raw native inventory")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = analyse(read_terminal(args.receipt))
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
