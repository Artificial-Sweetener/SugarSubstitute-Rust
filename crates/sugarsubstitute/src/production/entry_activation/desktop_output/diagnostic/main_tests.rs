// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify compact closed producer facts and actual bounded receiving scratch behavior.
use super::super::{Diagnostic, Observation};
use super::*;
use sugarsubstitute_contracts::transcript::OutputStream;

const LINE: &str = "operation=\"main_failure_transition\" failed_operation=\"decode_main_observation\" launch=1 navigation=1 observation=Frame code=JsonData";

#[test]
fn main_grammar_rejects_unknown_extensions_noncanonical_identities_and_native_codes() {
    for operation in OPERATIONS {
        for observation in OBSERVATIONS {
            let line = LINE
                .replace("decode_main_observation", operation)
                .replace("observation=Frame", &format!("observation={observation}"));
            let actual = parse(line.as_bytes());
            assert_eq!(actual.map(|value| value.operation), Some(*operation));
            assert_eq!(actual.map(|value| value.observation), Some(*observation));
        }
    }
    for token in [
        "Hresult(-2147483648)",
        "Hresult(2147483647)",
        "Os(233)",
        "Io(TimedOut)",
        "Readiness(ContextLost)",
        "JsonIo",
        "JsonSyntax",
        "JsonData",
        "JsonEof",
        "Opaque",
    ] {
        assert!(parse(LINE.replace("JsonData", token).as_bytes()).is_some());
    }
    for bad in [
        LINE.replace("decode_main_observation", "PRIVATE"),
        LINE.replace("observation=Frame", "observation=Unknown"),
        LINE.replace("launch=1", "launch=0"),
        LINE.replace("launch=1", "launch=01"),
        LINE.replace("launch=1", "launch=18446744073709551616"),
        LINE.replace("navigation=1", "navigation=-1"),
        LINE.replace("JsonData", "Hresult(2147483648)"),
        LINE.replace("JsonData", "Os(-0)"),
        LINE.replace("JsonData", "Os(+1)"),
        LINE.replace("JsonData", "Readiness(PRIVATE)"),
        LINE.replace("JsonData", "Io(PRIVATE)"),
        LINE.replace("JsonData", "JsonData(PRIVATE)"),
        format!("{LINE} launch=2"),
        format!("{LINE} cause=PRIVATE"),
        format!("operation=\"other\" {LINE}"),
    ] {
        assert!(parse(bad.as_bytes()).is_none(), "Unexpected accepted malformed closed grammar");
    }
    assert!(parse(format!("{}{LINE}", "x".repeat(513)).as_bytes()).is_none());
}

#[test]
fn main_oversize_gap_eof_and_stdout_cannot_stitch_or_publish_private_suffixes() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stdout, 1, false, false, format!("{LINE}\n").as_bytes(), |value| {
        seen.push(value)
    });
    let bad = format!("{}{LINE}\n", "x".repeat(513));
    owner.feed(OutputStream::Stderr, 1, false, false, bad.as_bytes(), |value| seen.push(value));
    let split = LINE.len() / 2;
    owner.feed(OutputStream::Stderr, 2, false, false, &LINE.as_bytes()[..split], |value| {
        seen.push(value)
    });
    owner.feed(
        OutputStream::Stderr,
        4,
        true,
        false,
        format!("{}\n", &LINE[split..]).as_bytes(),
        |value| seen.push(value),
    );
    owner.feed(OutputStream::Stderr, 5, false, true, LINE.as_bytes(), |value| seen.push(value));
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 6, false, false, &LINE.as_bytes()[..split], |value| {
        seen.push(value)
    });
    owner.feed(
        OutputStream::Stderr,
        7,
        false,
        false,
        format!("{}\r\n", &LINE[split..]).as_bytes(),
        |value| seen.push(value),
    );
    owner.feed(OutputStream::Stderr, 8, false, false, format!("{LINE}\n").as_bytes(), |value| {
        seen.push(value)
    });
    assert_eq!(
        seen,
        vec![Observation::MainFailure(MainFailure {
            launch: 1,
            navigation: 1,
            operation: "decode_main_observation",
            observation: "Frame",
            code: Code::JsonData,
        })]
    );
}

#[test]
fn main_and_generic_failures_both_survive_saturated_progress_in_either_order() {
    for main_first in [false, true] {
        let mut owner = Diagnostic::new();
        let mut seen = Vec::new();
        for sequence in 1..=80 {
            let line = format!(
                "operation=\"child_preparation_transition\" capture_admitted=true capture=false registered=false configuration=false original_failure=None capture_failure=None failure_retained={}\n",
                sequence % 2 == 0
            );
            owner.feed(OutputStream::Stderr, sequence, false, false, line.as_bytes(), |value| {
                seen.push(value)
            });
        }
        assert_eq!(seen.len(), 30);
        let generic = "operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other";
        for (index, line) in
            if main_first { [LINE, generic] } else { [generic, LINE] }.into_iter().enumerate()
        {
            owner.feed(
                OutputStream::Stderr,
                81 + index as u64,
                false,
                false,
                format!("{line}\n").as_bytes(),
                |value| seen.push(value),
            );
        }
        assert_eq!(seen.len(), 32);
        assert!(seen.iter().any(|value| matches!(value, Observation::MainFailure(_))));
        assert!(seen.iter().any(|value| matches!(value, Observation::Failure(_))));
        assert_eq!(owner.bytes.len(), 512);
    }
}
