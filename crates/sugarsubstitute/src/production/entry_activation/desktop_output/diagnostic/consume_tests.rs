// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify consumer-only bounded grammar, actual text formatting and the unchanged reservation.
use super::super::{Diagnostic, Observation, parse_failure};
use super::*;
use sugarsubstitute_contracts::transcript::OutputStream;
const LINE: &str = "operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other consume_stage=ChildDelivery category=Bootstrap(Transport)";
#[test]
fn consumer_failure_grammar_rejects_foreign_observation_fields_unknown_categories_duplicates_and_oversize()
 {
    let actual = parse_failure(LINE.as_bytes()).and_then(|f| f.consume);
    assert_eq!(
        actual,
        Some(ConsumeFailureDetail {
            stage: Stage::ChildDelivery,
            code: Code::Bootstrap(sugarsubstitute_desktop::StartupBootstrapCause::Transport)
        })
    );
    for bad in [
        LINE.replace("PollFrame", "ObserveStartup"),
        LINE.replace("PollFrame", "Cleanup"),
        LINE.replace("kind=Other", "kind=Io"),
        LINE.replace("ChildDelivery", "PRIVATE"),
        LINE.replace("Transport", "PRIVATE"),
        LINE.replace("category=", "code="),
        format!("{LINE} observation=CloseRequested"),
        format!("{LINE} consume_stage=Retry"),
        format!("{LINE} cause=PRIVATE"),
        LINE.replace(" category=Bootstrap(Transport)", ""),
        format!("operation=\"foreign\" {LINE}"),
        format!("{}{LINE}", "x".repeat(513)),
    ] {
        assert!(parse_failure(bad.as_bytes()).is_none());
    }
    for code in [
        "Bootstrap(State)",
        "Bootstrap(Identity)",
        "Bootstrap(Capture)",
        "Bootstrap(Configuration)",
        "Bootstrap(Deadline)",
        "NativeOs(CounterQuery,4294967295)",
        "Os(-2147483648)",
        "Request(Deadline)",
        "Opaque",
        "Transcript(ZeroIdentity)",
        "Transcript(Bound)",
        "Transcript(EmptyGap)",
        "Transcript(InvalidStyle)",
        "Transcript(ForeignResource)",
        "Transcript(Sequence)",
        "Transcript(StreamEnded)",
        "Transcript(Exhausted)",
        "Transcript(SnapshotCapacity)",
    ] {
        assert!(parse_failure(LINE.replace("Bootstrap(Transport)", code).as_bytes()).is_some());
    }
    for code in [
        "Bootstrap(Transport,1)",
        "Bootstrap(Unknown)",
        "Transcript(Unknown)",
        "Transcript(Sequence,1)",
        "Transcript(Sequence(secret))",
        "Transcript(sequence)",
        "NativeOs(CounterQuery,4294967296)",
        "Os(+1)",
        "Os(-0)",
    ] {
        assert!(parse_failure(LINE.replace("Bootstrap(Transport)", code).as_bytes()).is_none());
    }
}
#[test]
fn consumer_failure_reuses_first_generic_slot_with_unchanged_progress_and_main_reservations() {
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    for sequence in 1..=80 {
        let progress = format!(
            "operation=\"child_preparation_transition\" channel_phase=Authenticated channel_terminal=None capture_input=Transferred capture_admitted=true capture=false registered=false configuration=false original_failure=None capture_failure=None failure_retained={}\n",
            sequence % 2 == 0
        );
        owner.feed(OutputStream::Stderr, sequence, false, false, progress.as_bytes(), |v| {
            seen.push(v)
        });
    }
    assert_eq!(seen.len(), 30);
    let closed = format!("{LINE}\n");
    for sequence in [81, 82] {
        owner.feed(OutputStream::Stderr, sequence, false, false, closed.as_bytes(), |v| {
            seen.push(v)
        });
    }
    let main=b"operation=\"main_failure_transition\" failed_operation=\"decode_main_observation\" launch=1 navigation=1 observation=Probe code=JsonData\n";
    owner.feed(OutputStream::Stderr, 83, false, false, main, |v| seen.push(v));
    assert_eq!(seen.len(), 32);
    assert_eq!(owner.bytes.len(), 512);
    assert_eq!(seen.iter().filter(|v| matches!(v, Observation::Failure(_))).count(), 1);
    assert!(matches!(seen[30],Observation::Failure(f) if f.consume.is_some()&&f.startup.is_none()));
    assert!(matches!(seen[31], Observation::MainFailure(_)));
}

#[test]
fn actual_production_consumer_formatter_roundtrips_closed_stage_category_within_original_line_bound()
-> Result<(), Box<dyn std::error::Error>> {
    // Every stage the desktop can name, including the canvas's own delivery stage.
    for &stage in Stage::ALL {
        use sugarsubstitute_contracts::transcript::TranscriptError as Transcript;
        for category in [
            Code::Native(
                sugarsubstitute_desktop::StartupNativeCause::CounterConversion,
                Some(u32::MAX),
            ),
            Code::Transcript(Transcript::ZeroIdentity),
            Code::Transcript(Transcript::Bound),
            Code::Transcript(Transcript::EmptyGap),
            Code::Transcript(Transcript::InvalidStyle),
            Code::Transcript(Transcript::ForeignResource),
            Code::Transcript(Transcript::Sequence),
            Code::Transcript(Transcript::StreamEnded),
            Code::Transcript(Transcript::Exhausted),
            Code::Transcript(Transcript::SnapshotCapacity),
            Code::CanvasMessage(sugarsubstitute_desktop::StartupCanvasMessageCause::Storage),
        ] {
            let bytes = super::super::tests::formatted_owner_diagnostics(|| {
                tracing::warn!(operation="desktop_failure_transition",boundary=%"PollFrame",kind=%"Other",consume_stage=?stage,category=%category,"First owned desktop failure retained");
            })?;
            assert!(bytes.starts_with(b"2026-10-13T"));
            assert!(bytes.split(|byte| *byte == b'\n').all(|line| line.len() <= 512));
            let mut owner = Diagnostic::new();
            let mut seen = Vec::new();
            owner.feed(OutputStream::Stderr, 1, false, false, &bytes, |v| seen.push(v));
            assert_eq!(seen.len(), 1);
            assert!(
                matches!(seen[0],Observation::Failure(f) if f.startup.is_none()&&f.consume==Some(ConsumeFailureDetail{stage,code:category}))
            );
        }
    }
    Ok(())
}
