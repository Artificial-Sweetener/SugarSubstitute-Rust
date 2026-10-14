// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify exact enriched first-failure grammar under unchanged scratch and observation limits.
use super::super::{Diagnostic, Observation, parse_failure};
use super::*;
use sugarsubstitute_contracts::transcript::OutputStream;

const LINE: &str = "operation=\"desktop_failure_transition\" boundary=ObserveStartup kind=Other observation=CloseRequested close_stage=CaptureObservation capture_snapshot=CounterFailed code=Native(CounterConversion)";

#[test]
fn startup_failure_grammar_keeps_closed_facts_and_rejects_duplicates_foreign_or_raw_tokens() {
    let actual = parse_failure(LINE.as_bytes()).and_then(|failure| failure.startup);
    assert_eq!(
        actual,
        Some(StartupFailureDetail {
            observation: "CloseRequested",
            stage: Stage::CaptureObservation,
            capture: Capture::CounterFailed,
            code: Code::Native(Native::CounterConversion, None)
        })
    );
    for bad in [
        LINE.replace("CloseRequested", "Unknown"),
        LINE.replace("CaptureObservation", "PRIVATE"),
        LINE.replace("CounterFailed", "Unknown"),
        LINE.replace("CounterConversion", "PRIVATE"),
        LINE.replace("ObserveStartup", "PollFrame"),
        LINE.replace("kind=Other", "kind=Io"),
        format!("{LINE} observation=NativeRetired"),
        format!("{LINE} cause=PRIVATE"),
        LINE.replace("capture_snapshot=CounterFailed ", ""),
        format!("operation=\"foreign\" {LINE}"),
        format!("{}{LINE}", "x".repeat(513)),
    ] {
        assert!(parse_failure(bad.as_bytes()).is_none());
    }
    assert!(
        parse_failure(
            b"operation=\"desktop_failure_transition\" boundary=ObserveStartup kind=Other"
        )
        .is_some()
    );
}

#[test]
fn startup_failure_codes_reject_noncanonical_overflow_and_unknown_native_or_request_categories() {
    for good in [
        "NativeOs(CounterQuery,4294967295)",
        "Os(-2147483648)",
        "Hresult(2147483647)",
        "Request(Deadline)",
        "Native(CaptureClock)",
        "Opaque",
        "TimedOut",
        "Disconnected",
        "Denied",
        "InvalidInput",
        "Io",
    ] {
        assert!(
            parse_failure(LINE.replace("Native(CounterConversion)", good).as_bytes()).is_some()
        );
    }
    for bad in [
        "NativeOs(CounterQuery,+1)",
        "NativeOs(CounterQuery,01)",
        "NativeOs(CounterQuery,4294967296)",
        "NativeOs(PRIVATE,1)",
        "NativeOs(CounterQuery,1,2)",
        "Os(+1)",
        "Os(-0)",
        "Os(2147483648)",
        "Hresult(-2147483649)",
        "Request(PRIVATE)",
        "Native(PRIVATE)",
        "Io(PRIVATE)",
    ] {
        assert!(parse_failure(LINE.replace("Native(CounterConversion)", bad).as_bytes()).is_none());
    }
}

#[test]
fn enriched_startup_failure_reuses_one_first_failure_slot_after_saturated_progress() {
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
    owner.feed(OutputStream::Stderr, 81, false, false, format!("{LINE}\n").as_bytes(), |value| {
        seen.push(value)
    });
    owner.feed(OutputStream::Stderr, 82, false, false, format!("{LINE}\n").as_bytes(), |value| {
        seen.push(value)
    });
    let main = "operation=\"main_failure_transition\" failed_operation=\"decode_main_observation\" launch=1 navigation=1 observation=Probe code=JsonData\n";
    owner.feed(OutputStream::Stderr, 83, false, false, main.as_bytes(), |value| seen.push(value));
    assert_eq!(seen.len(), 32);
    assert_eq!(seen.iter().filter(|value| matches!(value, Observation::Failure(_))).count(), 1);
    assert!(matches!(seen[30],Observation::Failure(failure) if failure.startup.is_some()));
    assert!(matches!(seen[31], Observation::MainFailure(_)));
    assert_eq!(owner.bytes.len(), 512);
}

#[test]
fn startup_failure_unknown_oversize_gap_eof_and_stdout_cannot_publish_or_stitch_fields() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let split = LINE.len() / 2;
    owner.feed(OutputStream::Stdout, 1, false, false, format!("{LINE}\n").as_bytes(), |value| {
        seen.push(value)
    });
    owner.feed(
        OutputStream::Stderr,
        1,
        false,
        false,
        format!("{}{LINE}\n", "x".repeat(513)).as_bytes(),
        |value| seen.push(value),
    );
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
    assert_eq!(seen.len(), 1);
    assert!(matches!(seen[0],Observation::Failure(failure) if failure.startup.is_some()));
}

#[test]
fn actual_main_absence_failure_enrichment_uses_its_own_closed_observation_token() {
    let line = LINE.replace("observation=CloseRequested", "observation=MainAbsent");
    let actual = parse_failure(line.as_bytes()).and_then(|failure| failure.startup);
    assert_eq!(actual.map(|facts| facts.observation), Some("MainAbsent"));
    assert_eq!(parse_failure(line.replace("MainAbsent", "MainAbsenceClaim").as_bytes()), None);
}
