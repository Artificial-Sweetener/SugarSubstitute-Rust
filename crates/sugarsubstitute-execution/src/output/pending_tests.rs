// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish actual external pending issue results from requests and polls.
use super::*;
fn await_report(
    rig: &Rig,
    session: &WindowsOutputSession,
    accept: impl Fn(WindowsOutputReport) -> bool,
) -> TestResult<WindowsOutputReport> {
    loop {
        let count = rig.wake.count();
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        let report = session.control.snapshot();
        if accept(report) {
            return Ok(report);
        }
        rig.wake.wait_after(count)?;
    }
}
#[test]
fn immediate_reads_and_eof_never_claim_actual_pending_issue() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    lock(&boundary.data)[0].push_back(b"immediate".to_vec());
    for eof in &boundary.eof {
        eof.store(true, Ordering::Release);
    }
    let mut session = session(&rig, &boundary)?;
    let JobOutcome::Success(report) = completion(&rig, &mut session)?.outcome else {
        return Err("Expected actual EOF".into());
    };
    assert_eq!(report.pending_read_issues, [0, 0]);
    assert_eq!(report.pending_reads, [false, false]);
    assert_eq!(report.eof, [true, true]);
    Ok(())
}
#[test]
fn incomplete_polls_never_increment_actual_pending_issue_counts() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    await_report(&rig, &session, |r| r.pending_read_issues == [1, 1])?;
    boundary.wake(0)?;
    boundary.push(0, b"observed result".to_vec())?;
    let report = await_report(&rig, &session, |r| r.pending_read_issues[0] == 2)?;
    assert_eq!(report.pending_read_issues, [2, 1]);
    assert_eq!(report.pending_reads, [true, true]);
    session.control.close();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    let report = session.control.snapshot();
    assert_eq!(report.pending_reads, [false, false]);
    assert_eq!(report.pending_read_issues, [2, 1]);
    assert_eq!(report.eof, [false, false]);
    Ok(())
}
#[test]
fn cancellation_retains_pending_evidence_until_checked_retirement_not_request() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    await_report(&rig, &session, |r| r.pending_reads == [true, true])?;
    let pin = session.control.take_child_stdio()?;
    session.control.close();
    boundary.retiring.wait()?;
    let report = session.control.snapshot();
    assert_eq!(report.pending_reads, [true, true]);
    assert!(!report.retired);
    assert!(session.completion.try_take_completion().is_none());
    drop(pin);
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    let report = session.control.snapshot();
    assert_eq!(report.pending_read_issues, [1, 1]);
    assert_eq!(report.pending_reads, [false, false]);
    assert_eq!(report.eof, [false, false]);
    Ok(())
}
#[test]
fn exhausted_counter_rejects_next_issue_before_external_read_and_preserves_loss() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    await_report(&rig, &session, |r| r.pending_reads == [true, true])?;
    lock(&session.control.shared.state).pending_read_issues[0] = u64::MAX;
    boundary.push(0, b"last actual result".to_vec())?;
    let JobOutcome::Failure(error) = completion(&rig, &mut session)?.outcome else {
        return Err("Expected explicit counter exhaustion".into());
    };
    assert_eq!(error.stage(), "output_pending_issue_exhausted");
    assert_eq!(lock(&boundary.order).as_slice(), [0]);
    let report = session.control.snapshot();
    assert_eq!(report.pending_read_issues, [u64::MAX, 1]);
    assert_eq!(report.pending_reads, [false, false]);
    assert_eq!(report.eof, [false, false]);
    assert_eq!(report.counters[0].read_bytes, 18);
    Ok(())
}
