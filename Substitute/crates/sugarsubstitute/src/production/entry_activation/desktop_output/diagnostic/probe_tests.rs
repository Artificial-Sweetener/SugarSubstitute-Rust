// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual Desktop formatter and sole Root-owned stderr decoder.
use super::*;
use sugarsubstitute_desktop::frontend_probe_diagnostic::{Record, Stage, emit};

fn record(stage: Stage) -> Record {
    Record {
        launch: 1,
        navigation: 2,
        probe_operation: 3,
        issued: 3,
        completed: 3,
        blocked: if stage == Stage::Incomplete || stage == Stage::Timeout { 6144 } else { 0 },
        stage,
    }
}

#[test]
fn actual_desktop_probe_formatter_roundtrips_through_existing_root_stderr_owner()
-> Result<(), Box<dyn std::error::Error>> {
    let actual = record(Stage::Incomplete);
    let bytes = formatted_owner_diagnostics(|| emit(actual))?;
    assert!(bytes.split(|byte| *byte == b'\n').all(|line| line.len() <= 512));
    let mut decoder = Diagnostic::new();
    let mut seen = Vec::new();
    let split = bytes.len() / 2;
    decoder.feed(OutputStream::Stderr, 1, false, false, &bytes[..split], |v| seen.push(v));
    assert!(seen.is_empty());
    decoder.feed(OutputStream::Stderr, 2, false, false, &bytes[split..], |v| seen.push(v));
    assert_eq!(seen, [Observation::Probe(actual)]);
    Ok(())
}

#[test]
fn probe_cap_preserves_original_failure_slots_and_one_final_expiry_summary()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = formatted_owner_diagnostics(|| {
        for _ in 0..64 {
            emit(record(Stage::Incomplete));
        }
        emit(record(Stage::Timeout));
        emit(record(Stage::Timeout));
        tracing::warn!(operation="main_failure_transition",failed_operation="frontend_budget_expired",
            launch=1,navigation=2,observation=%"None",code=%"Io(TimedOut)",
            wait=%"Document",probe_operation=%"None");
    })?;
    let mut decoder = Diagnostic::new();
    let mut seen = Vec::new();
    decoder.feed(OutputStream::Stderr, 1, false, false, &bytes, |v| seen.push(v));
    assert_eq!(seen.iter().filter(|v| matches!(v, Observation::Probe(_))).count(), 17);
    assert_eq!(seen.len(), 18);
    assert_eq!(seen[16], Observation::Probe(record(Stage::Timeout)));
    assert!(matches!(seen[17], Observation::MainFailure(_)));
    Ok(())
}

#[test]
fn probe_framing_retains_existing_stdout_gap_oversize_and_replay_rejection()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = formatted_owner_diagnostics(|| emit(record(Stage::Incomplete)))?;
    let mut decoder = Diagnostic::new();
    let mut seen = Vec::new();
    decoder.feed(OutputStream::Stdout, 1, false, false, &bytes, |v| seen.push(v));
    let split = bytes.len() / 2;
    decoder.feed(OutputStream::Stderr, 1, false, false, &bytes[..split], |v| seen.push(v));
    decoder.feed(OutputStream::Stderr, 3, true, false, &bytes[split..], |v| seen.push(v));
    assert!(seen.is_empty());
    decoder.feed(OutputStream::Stderr, 4, false, false, &[b'x'; 513], |v| seen.push(v));
    decoder.feed(OutputStream::Stderr, 5, false, false, &bytes, |v| seen.push(v));
    assert!(seen.is_empty());
    decoder.feed(OutputStream::Stderr, 6, false, false, &bytes, |v| seen.push(v));
    decoder.feed(OutputStream::Stderr, 6, false, false, &bytes, |v| seen.push(v));
    assert_eq!(seen, [Observation::Probe(record(Stage::Incomplete))]);
    Ok(())
}
