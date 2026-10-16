// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep genuine final bytes independent of close intent and actual native retirement.
use super::*;
fn pending(rig: &Rig, session: &WindowsOutputSession) -> TestResult {
    loop {
        let count = rig.wake.count();
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if session.control.snapshot().pending_reads == [true; 2] {
            return Ok(());
        }
        rig.wake.wait_after(count)?;
    }
}
#[test]
fn cancelling_at_close_intent_cannot_observe_a_subsequent_native_finished_tail() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut actual = session(&rig, &boundary)?;
    pending(&rig, &actual)?;
    actual.control.close();
    assert!(matches!(completion(&rig, &mut actual)?.outcome, JobOutcome::Cancelled));
    boundary.push(1, b"actual final shutdown return\n".to_vec())?;
    assert!(actual.control.take_batch()?.is_none());
    assert_eq!(actual.control.snapshot().counters[1].read_bytes, 0);
    assert_eq!(actual.control.snapshot().eof, [false; 2]);
    assert_eq!(lock(&boundary.data)[1].len(), 1);
    Ok(())
}
#[test]
fn actual_late_tail_is_delivered_before_dual_eof_and_checked_retirement() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut actual = session(&rig, &boundary)?;
    pending(&rig, &actual)?;
    let hold = RetirementHold::new(&boundary);
    let tail = b"actual final shutdown return\n";
    boundary.push(1, tail.to_vec())?;
    boundary.end(0)?;
    boundary.end(1)?;
    boundary.retiring.wait()?;
    assert!(actual.completion.try_take_completion().is_none());
    assert!(!actual.control.snapshot().retired);
    let mut observed = Vec::new();
    let mut eof = [false; 2];
    while let Some(guard) = actual.control.take_batch()? {
        let batch = guard.batch().ok_or("Actual output batch missing")?;
        if batch.stream() == OutputStream::Stderr {
            observed.extend_from_slice(batch.bytes());
        }
        eof[batch.stream().index()] |= batch.end();
    }
    assert_eq!(observed, tail);
    assert_eq!(eof, [true; 2]);
    drop(hold);
    let JobOutcome::Success(report) = completion(&rig, &mut actual)?.outcome else {
        return Err("Natural dual EOF must retain successful checked retirement".into());
    };
    assert!(report.retired);
    assert_eq!(report.eof, [true; 2]);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn final_eof_does_not_hide_a_genuine_checked_retirement_error() -> TestResult {
    use std::error::Error;
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let hold = RetirementHold::new(&boundary);
    boundary.eof[0].store(true, Ordering::Release);
    boundary.eof[1].store(true, Ordering::Release);
    let mut actual = session(&rig, &boundary)?;
    boundary.retiring.wait()?;
    assert!(actual.completion.try_take_completion().is_none());
    // Preparation drains execution notices, so inject the checked failure only after it settles.
    boundary.fail_retire.store(1, Ordering::Release);
    boundary.wake(0)?;
    let observed_failure = loop {
        let count = rig.wake.count();
        if let Err(cause) = rig.client.drain_completions(NonZeroUsize::MIN) {
            break cause;
        }
        assert!(actual.completion.try_take_completion().is_none());
        rig.wake.wait_after(count)?;
    };
    assert_eq!(observed_failure.operation(), Operation::NativeLifecycle);
    let mut source: &(dyn Error + 'static) = &observed_failure;
    let native = loop {
        if let Some(native) = source.downcast_ref::<NativeObservationError>() {
            break native;
        }
        source = source.source().ok_or("Original checked retirement cause missing")?;
    };
    assert_eq!(native.stage(), "output_checked_retirement");
    assert_eq!(boundary.fail_retire.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert!(!actual.control.snapshot().retired);
    assert_eq!(actual.control.snapshot().eof, [true; 2]);
    assert_eq!(lock(&rig.client.core.state).bytes, MIN_WINDOWS_OUTPUT_BYTES);
    assert!(actual.completion.try_take_completion().is_none());
    // A failed checked close retains its owner. Only this later external readiness change retries it.
    drop(hold);
    let JobOutcome::Failure(cause) = completion(&rig, &mut actual)?.outcome else {
        return Err("Original checked retirement failure must remain a failure".into());
    };
    assert_eq!(cause.stage(), "output_checked_retirement");
    assert!(std::ptr::eq(
        native.source().ok_or("Observed original cause missing")?,
        cause.source().ok_or("Terminal original cause missing")?,
    ));
    assert!(actual.control.snapshot().retired);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(actual.control.snapshot().eof, [true; 2]);
    Ok(())
}
