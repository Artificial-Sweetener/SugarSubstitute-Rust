// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real HTTP owner against deterministic external acquisition and closure fakes.
use super::*;
#[path = "ownership/boundary.rs"]
mod boundary;
#[path = "ownership/harness.rs"]
mod harness;
use harness::Harness;
use std::error::Error;
use sugarsubstitute_contracts::execution::{JobPhase, TerminalKind};
type TestResult = Result<(), Box<dyn Error>>;
#[test]
fn partial_begin_failure_retains_each_native_slot_and_original_memory_until_real_cleanup()
-> TestResult {
    for slots in 1..=3 {
        let mut harness = Harness::new(slots, false, false, false)?;
        harness.submit()?;
        harness.terminal()?;
        let error = harness.owner.take_readiness().err().ok_or("Expected begin failure")?;
        assert_eq!(error.stage(), "fake_begin");
        assert_eq!(
            error.source().ok_or("Missing original cause")?.to_string(),
            "original-begin-cause"
        );
        assert_eq!(
            harness.retained(),
            (
                1,
                if slots > 1 { 2 } else { 0 },
                if slots > 2 { 3 } else { 0 },
                READINESS_TRANSPORT_BYTES - 128 * 1024,
                500
            )
        );
        assert_eq!(harness.attempt.snapshot().reserved_bytes, READINESS_TRANSPORT_BYTES);
        assert!(lock(&harness.boundary.requests).is_empty());
        assert!(matches!(harness.cleanup()?.outcome, JobOutcome::Success(())));
        assert!(lock(&harness.owner.shared.transaction).is_none());
        assert_eq!(harness.attempt.snapshot().reserved_bytes, 0);
        let expected = match slots {
            1 => vec![1],
            2 => vec![2, 1],
            _ => vec![3, 2, 1],
        };
        assert_eq!(*lock(&harness.boundary.closed), expected);
        assert!(matches!(harness.cleanup()?.outcome, JobOutcome::Success(())));
        assert_eq!(*lock(&harness.boundary.closed), expected);
        harness.finish()?;
    }
    Ok(())
}
#[test]
fn late_partial_result_keeps_custody_after_actual_timed_out_completion() -> TestResult {
    let mut harness = Harness::new(3, true, false, false)?;
    harness.submit()?;
    harness.terminal()?;
    assert!(matches!(
        harness.owner.pending.as_ref().ok_or("Missing job")?.status().phase,
        JobPhase::Terminal(TerminalKind::TimedOut)
    ));
    assert_eq!(
        harness.owner.take_readiness().err().ok_or("Expected late rejection")?.stage(),
        "attempt_terminal"
    );
    assert_eq!(harness.retained(), (1, 2, 3, READINESS_TRANSPORT_BYTES - 128 * 1024, 500));
    assert_eq!(harness.attempt.snapshot().reserved_bytes, READINESS_TRANSPORT_BYTES);
    assert!(matches!(harness.cleanup()?.outcome, JobOutcome::Success(())));
    assert_eq!(*lock(&harness.boundary.phases), [("begin", 500), ("close", 5000)]);
    harness.finish()
}
#[test]
fn cancelled_inflight_attempt_returns_only_after_boundary_and_keeps_partial_slots() -> TestResult {
    let mut harness = Harness::new(0, false, true, false)?;
    harness.submit()?;
    harness.boundary.entered()?;
    harness.owner.pending.as_ref().ok_or("Missing attempt")?.cancel();
    assert!(!matches!(
        harness.owner.pending.as_ref().ok_or("Missing attempt")?.status().phase,
        JobPhase::Terminal(_)
    ));
    harness.boundary.release();
    harness.terminal()?;
    assert!(matches!(
        harness.owner.pending.as_ref().ok_or("Missing attempt")?.status().phase,
        JobPhase::Terminal(TerminalKind::Cancelled)
    ));
    assert_eq!(
        harness.owner.take_readiness().err().ok_or("Expected cancelled rejection")?.stage(),
        "attempt_terminal"
    );
    assert_eq!(harness.retained(), (1, 2, 3, READINESS_TRANSPORT_BYTES - 128 * 1024, 500));
    assert!(lock(&harness.boundary.requests).is_empty());
    harness.finish()
}
#[test]
fn delivery_after_original_effective_deadline_rejects_actual_success_without_new_budget()
-> TestResult {
    let mut harness = Harness::new(0, false, false, false)?;
    harness.submit()?;
    harness.terminal()?;
    assert!(matches!(
        harness.owner.pending.as_ref().ok_or("Missing attempt")?.status().phase,
        JobPhase::Terminal(TerminalKind::Success)
    ));
    assert_eq!(harness.retained(), (0, 0, 0, READINESS_TRANSPORT_BYTES - 128 * 1024, 500));
    assert_eq!(
        *lock(&harness.boundary.requests),
        ["/system_stats", "/extensions", "/index.html", "/extensions/SugarCubes/main.js"]
    );
    harness.clock.0.store(500, std::sync::atomic::Ordering::Release);
    assert_eq!(
        harness.owner.take_readiness().err().ok_or("Expected late delivery")?.stage(),
        "attempt_delivery_or_retirement"
    );
    assert_eq!(harness.attempt.snapshot().reserved_bytes, READINESS_TRANSPORT_BYTES);
    assert!(lock(&harness.boundary.phases).iter().all(|(_, deadline)| *deadline == 500));
    harness.finish()
}
#[test]
fn rejected_finite_cleanup_preserves_native_custody_and_memory_until_admitted_close() -> TestResult
{
    let mut harness = Harness::new(3, false, false, false)?;
    harness.submit()?;
    harness.terminal()?;
    assert!(harness.owner.take_readiness().is_err());
    let rejected = harness.client.create_scope(
        sugarsubstitute_contracts::execution::ScopeSpec {
            id: sugarsubstitute_contracts::execution::ScopeId::new(4)?,
            deadline: None,
        },
        None,
    )?;
    rejected.cancel();
    let before = harness.retained();
    let spec = harness.spec(5000)?;
    assert!(harness.owner.submit_close(&harness.client, &rejected, spec).is_err());
    assert_eq!(harness.retained(), before);
    assert_eq!(harness.attempt.snapshot().reserved_bytes, READINESS_TRANSPORT_BYTES);
    assert!(lock(&harness.boundary.closed).is_empty());
    harness.finish()
}
#[test]
fn failed_close_preserves_remaining_slots_cause_charge_and_original_attempt_deadline() -> TestResult
{
    let mut harness = Harness::new(3, false, false, true)?;
    harness.submit()?;
    harness.terminal()?;
    assert!(harness.owner.take_readiness().is_err());
    let JobOutcome::Failure(error) = harness.cleanup()?.outcome else {
        return Err("Expected close failure".into());
    };
    assert_eq!(error.stage(), "fake_close");
    assert_eq!(error.source().ok_or("Missing original cause")?.to_string(), "original-close-cause");
    assert_eq!(harness.retained(), (1, 2, 0, READINESS_TRANSPORT_BYTES - 128 * 1024, 500));
    assert_eq!(harness.attempt.snapshot().reserved_bytes, READINESS_TRANSPORT_BYTES);
    assert_eq!(*lock(&harness.boundary.closed), [3]);
    assert!(matches!(harness.cleanup()?.outcome, JobOutcome::Success(())));
    assert_eq!(*lock(&harness.boundary.closed), [3, 2, 1]);
    assert_eq!(harness.attempt.snapshot().reserved_bytes, 0);
    assert_eq!(*lock(&harness.boundary.phases), [("begin", 500), ("close", 5000), ("close", 5000)]);
    harness.finish()
}
#[test]
fn pending_attempt_and_missing_deadline_reject_cleanup_before_any_external_close() -> TestResult {
    let mut harness = Harness::new(0, false, true, false)?;
    harness.submit()?;
    harness.boundary.entered()?;
    let spec = harness.spec(5000)?;
    assert!(harness.owner.submit_close(&harness.client, &harness.cleanup, spec).is_err());
    harness.owner.pending.as_ref().ok_or("Missing attempt")?.cancel();
    harness.boundary.release();
    harness.terminal()?;
    assert!(harness.owner.take_readiness().is_err());
    let mut spec = harness.spec(5000)?;
    spec.deadline = None;
    assert!(matches!(
        harness.owner.submit_close(&harness.client, &harness.cleanup, spec),
        Err(AdmissionError::DeadlineRequired)
    ));
    assert!(lock(&harness.boundary.closed).is_empty());
    harness.finish()
}
