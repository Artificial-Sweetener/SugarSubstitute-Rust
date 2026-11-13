// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual compact producers, stream framing, safe relay and original cap reservations.
use super::super::{Diagnostic, Observation};
use super::*;
use sugarsubstitute_contracts::{lifecycle::*, transcript::OutputStream};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn bytes(emit: impl FnOnce()) -> Result<Vec<u8>> {
    super::super::tests::formatted_owner_diagnostics(emit)
}
#[test]
fn actual_close_producers_decode_fragmented_and_relay_exact_cutoffs_without_run_payload() -> Result
{
    let prepared = Prepared {
        original_ns: u64::MAX,
        source: DeadlineSource::AttemptCleanup,
        phase: Some(StartupPhase::Running),
        attempt_present: true,
        attempt_complete: false,
        shutdown_budget_present: false,
    };
    let records = [
        CloseTrace::Control {
            stage: ControlStage::Prepared,
            control: Control::MainRetired,
            request: u64::MAX,
            deadline_qpc: i64::MAX,
            prepared: Some(prepared),
        },
        CloseTrace::Control {
            stage: ControlStage::ReplyConsumed,
            control: Control::MainRetired,
            request: u64::MAX,
            deadline_qpc: i64::MAX,
            prepared: None,
        },
        CloseTrace::Budget { stage: BudgetStage::CapturePending, moments: None },
        CloseTrace::Budget { stage: BudgetStage::CaptureLatched, moments: Some([1, 10, 15]) },
    ];
    let actual = bytes(|| {
        for record in records {
            record.emit();
        }
    })?;
    assert!(actual.split(|b| *b == b'\n').all(|line| line.len() <= 512));
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    let split = actual.len() / 2;
    owner.feed(OutputStream::Stderr, 1, false, false, &actual[..split], |v| seen.push(v));
    owner.feed(OutputStream::Stderr, 2, false, false, &actual[split..], |v| seen.push(v));
    let closed: Vec<_> = seen
        .into_iter()
        .filter_map(|v| if let Observation::Close(v) = v { Some(v) } else { None })
        .collect();
    assert_eq!(closed, records);
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let output = String::from_utf8(bytes(|| {
        for record in &closed {
            relay(run, *record);
        }
    })?)?;
    assert_eq!(output.matches("operation=\"desktop_child_close\"").count(), 4);
    assert!(output.contains("source: AttemptCleanup"));
    assert!(output.contains("phase: Some(Running)"));
    assert!(output.contains(&format!("run={run:?}")));
    assert!(output.contains(&format!("original_ns: {}", u64::MAX)));
    assert!(std::mem::size_of::<CloseTrace>() <= std::mem::size_of::<super::super::Failure>());
    Ok(())
}
#[test]
fn close_decoder_rejects_raw_extensions_and_preserves_existing_progress_failure_caps() -> Result {
    let valid = bytes(|| {
        CloseTrace::Control {
            stage: ControlStage::Admitted,
            control: Control::MainRetired,
            request: 1,
            deadline_qpc: 2,
            prepared: None,
        }
        .emit()
    })?;
    let text = String::from_utf8(valid.clone())?;
    for bad in [
        text.replace("request=1", "request=0"),
        text.replace("request=1", "request=01"),
        text.replace("stage=Admitted", "stage=Invented"),
        text.replace("control=MainRetired", "control=Activity"),
        text.replace("deadline_qpc=2", "deadline_qpc=2 raw=private"),
        text.replace("deadline_qpc=2", "deadline_qpc=-2"),
        text.replace("deadline_qpc=2", "deadline_qpc=9223372036854775808"),
        text.replace("request=1", "request=1 request=1"),
        format!("operation=\"unknown\" {text}"),
    ] {
        assert!(parse(bad.trim_end().as_bytes()).is_none());
    }
    let mut owner = Diagnostic::new();
    let mut seen = Vec::new();
    owner.feed(OutputStream::Stdout, 1, false, false, &valid, |v| seen.push(v));
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 1, false, false, &valid[..valid.len() - 1], |v| seen.push(v));
    assert!(seen.is_empty());
    owner.feed(OutputStream::Stderr, 2, true, false, b"\n", |v| seen.push(v));
    assert!(seen.is_empty());
    for sequence in 3..43 {
        owner.feed(OutputStream::Stderr, sequence, false, false, &valid, |v| seen.push(v));
    }
    assert_eq!(seen.len(), 30);
    owner.feed(
        OutputStream::Stderr,
        43,
        false,
        false,
        b"operation=\"desktop_failure_transition\" boundary=PollFrame kind=Other\n",
        |v| seen.push(v),
    );
    owner.feed(OutputStream::Stderr,44,false,false,b"operation=\"main_failure_transition\" failed_operation=\"decode_main_observation\" launch=1 navigation=1 observation=Frame code=JsonData\n",|v|seen.push(v));
    assert_eq!(seen.len(), 32);
    assert!(matches!(seen[30], Observation::Failure(_)));
    assert!(matches!(seen[31], Observation::MainFailure(_)));
    Ok(())
}
