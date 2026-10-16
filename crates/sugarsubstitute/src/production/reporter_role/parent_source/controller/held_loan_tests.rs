// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real Core deferral, causal loan return, immutable retry and original terminal/admission custody.
use super::*;
use crate::production::incident_host::CONTROLLER_BYTES;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{
        Condvar,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_execution::ExecutionOwner;

#[path = "held_loan_tests/failure_custody.rs"]
mod failure_custody;
#[path = "held_loan_tests/support.rs"]
mod support;
use support::*;
#[derive(Clone, Copy, Eq, PartialEq)]
enum Case {
    Deferred,
    Cancelled,
    Expired,
    Rejected,
    Capacity,
    RetryExpired,
    Mismatched,
}
fn delivered(client: &ExecutionClient, wake: &Wake, relay: &mut ReporterRelay) -> TestResult {
    loop {
        let before = wake.observed();
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(7))?;
        relay.capture(&batch)?;
        if relay.worker_delivered() {
            return Ok(());
        }
        wake.wait_after(before)?;
    }
}
fn held_loan(case: Case) -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(0)));
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut prepare = client.submit_blocking(
        &scope,
        specification(jobs.allocate()?, CONTROLLER_BYTES),
        |context| {
            let mut funding = Some(ReporterFunding::new(&context)?);
            Ok(ReporterSource::new(&mut funding)?)
        },
    )?;
    let prepared = terminal(&client, &wake, &mut prepare)?;
    let source = match prepared.outcome {
        JobOutcome::Success(source) => source,
        JobOutcome::Failure(cause) => return Err(cause),
        _ => return Err("Actual original controller admission did not succeed".into()),
    };
    drop(prepared.memory);
    drop(prepare);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    let mut relay = ReporterRelay::new(source);
    let gate = Arc::new(Gate::default());
    let _release = ReleaseOnDrop(gate.clone());
    let workers = owner.limits().blocking_workers;
    assert_eq!(workers, 2);
    let boundary = gate.clone();
    let mut blocker =
        client.submit_blocking(&scope, specification(jobs.allocate()?, 0), move |_| {
            boundary.enter()
        })?;
    let loan_boundary = Arc::new(LoanBoundary::default());
    let _release_loan = ReleaseLoanOnDrop(loan_boundary.clone());
    let held = loan_boundary.clone();
    let entered = gate.clone();
    let actual_source = relay.source().alias();
    let mut holder =
        client.submit_blocking(&scope, specification(jobs.allocate()?, 0), move |_| {
            held.run(actual_source, &entered)
        })?;
    gate.wait_entered(workers)?;
    let until = Moment::from_nanos(45_000_000_000);
    relay.submit(&client, &scope, &jobs, until)?;
    let RelayWork::Admitted(first_job) = &relay.work else {
        return Err("Actual Core job missing".into());
    };
    assert_eq!(first_job.status().phase, JobPhase::Queued);
    let first_id = first_job.spec().id;
    let original = (scope.owner_id(), scope.id(), until);
    assert_eq!(relay.original, Some(original));
    let alias = relay.source().alias();
    let mut loan = alias.loan()?;
    let pointer = std::ptr::from_ref(loan.state()?) as usize;
    assert!(loan.state()?.first.is_none());
    assert!(loan.state()?.core_scope.is_none());
    assert!(loan.state()?.funding.is_some());
    assert!(loan.state()?.parent.is_none());
    drop(loan);
    loan_boundary.acquire()?;
    assert!(matches!(alias.loan(), Err(ReporterSourceIssue::Busy)));
    match case {
        Case::Cancelled => relay.cancel(),
        Case::Expired => clock.0.store(until.as_nanos(), Ordering::Release),
        Case::Deferred
        | Case::Rejected
        | Case::Capacity
        | Case::RetryExpired
        | Case::Mismatched => {}
    }
    gate.release();
    if case == Case::Mismatched {
        mismatched_delivery(&client, &wake, &mut relay, first_id)?;
    } else {
        delivered(&client, &wake, &mut relay)?;
    }
    assert!(matches!(terminal(&client, &wake, &mut blocker)?.outcome, JobOutcome::Success(())));
    let expected = match case {
        Case::Cancelled => Some(TerminalKind::Cancelled),
        Case::Expired => Some(TerminalKind::TimedOut),
        Case::Mismatched => None,
        Case::Deferred | Case::Rejected | Case::Capacity | Case::RetryExpired => {
            Some(TerminalKind::Success)
        }
    };
    assert_eq!(relay.worker_terminal(), expected);
    assert_eq!(relay.worker_failure(), None);
    assert_eq!(
        match &relay.work {
            RelayWork::Delivered(done) => done.spec.deadline,
            _ => return Err("Actual Core completion missing".into()),
        },
        Some(until)
    );
    assert!(matches!(alias.loan(), Err(ReporterSourceIssue::Busy)));
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES + worker::REPORTER_CORE_BYTES);
    if matches!(case, Case::Deferred | Case::Rejected | Case::Capacity | Case::RetryExpired) {
        assert!(matches!(relay.take_delivery()?, ReporterDelivery::Deferred));
        assert!(relay.retry_pending());
        assert!(matches!(relay.take_delivery()?, ReporterDelivery::Pending));
        assert_eq!(relay.worker_terminal(), None);
        assert_eq!(relay.original, Some(original));
        assert_eq!(relay.deadline(), Some(until));
        assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
        let before = jobs.allocate()?.get();
        for _ in 0..16 {
            assert_eq!(relay.retry(&client, &scope, &jobs), Err(ReporterSourceIssue::Busy));
            assert!(matches!(relay.work, RelayWork::Idle));
            assert_eq!(relay.original, Some(original));
        }
        assert_eq!(jobs.allocate()?.get(), before + 1);
        assert_eq!(scope.snapshot().queued, 0);
        assert_eq!(scope.snapshot().running, 1);
        assert_eq!(relay.check_deadline(client.now()?), Ok(()));
        if case == Case::RetryExpired {
            clock.0.store(until.as_nanos(), Ordering::Release);
            assert_eq!(relay.check_deadline(client.now()?), Err(ReporterSourceIssue::Deadline));
            assert!(matches!(alias.loan(), Err(ReporterSourceIssue::Busy)));
            assert!(matches!(relay.work, RelayWork::Idle));
            assert_eq!(relay.original, Some(original));
            assert_eq!(scope.snapshot().running, 1);
            assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
        }
    } else {
        if case == Case::Mismatched {
            assert!(matches!(relay.take_delivery(), Err(ReporterSourceIssue::Identity)));
            assert!(!relay.matched);
            assert!(relay.work.take_failure(relay.matched).is_none());
            assert!(
                matches!(&relay.work, RelayWork::Delivered(done) if matches!(done.outcome, JobOutcome::Success(worker::RelayTurn::Deferred)))
            );
        } else {
            assert!(matches!(relay.take_delivery()?, ReporterDelivery::Pending));
        }
        assert_eq!(relay.worker_terminal(), expected);
        assert_eq!(relay.retry(&client, &scope, &jobs), Err(ReporterSourceIssue::Busy));
    }
    let before_return = wake.observed();
    loan_boundary.release();
    assert!(matches!(terminal(&client, &wake, &mut holder)?.outcome, JobOutcome::Success(())));
    assert!(wake.observed() > before_return);
    let mut returned = alias.loan()?;
    assert_eq!(std::ptr::from_ref(returned.state()?) as usize, pointer);
    assert!(returned.state()?.funding.is_some());
    drop(returned);
    match case {
        Case::Deferred => {
            relay.retry(&client, &scope, &jobs)?;
            let RelayWork::Admitted(retried) = &relay.work else {
                return Err("Actual retry job missing".into());
            };
            assert!(retried.spec().id.get() > first_id.get());
            assert_eq!(retried.scope_id(), scope.id());
            assert_eq!(retried.spec().deadline, Some(until));
            delivered(&client, &wake, &mut relay)?;
            assert!(matches!(relay.work, RelayWork::Delivered(_)));
            assert_eq!(relay.worker_terminal(), Some(TerminalKind::Failure));
            assert_eq!(relay.worker_failure(), Some(&ReporterSourceIssue::Busy));
            assert_eq!(alias.loan()?.state()?.first, Some(ReporterSourceIssue::Busy));
            assert_eq!(alias.loan()?.state()?.core_scope, Some(scope.id()));
            assert!(matches!(relay.take_delivery()?, ReporterDelivery::Pending));
            assert_eq!(relay.original, Some(original));
            assert!(!relay.retry_pending());
            clock.0.store(until.as_nanos(), Ordering::Release);
            assert_eq!(relay.check_deadline(client.now()?), Ok(()));
            assert_eq!(relay.worker_failure(), Some(&ReporterSourceIssue::Busy));
            assert_eq!(
                scope.snapshot().reserved_bytes,
                CONTROLLER_BYTES + worker::REPORTER_CORE_BYTES
            );
        }
        Case::Rejected => {
            let foreign =
                client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
            assert_eq!(relay.retry(&client, &foreign, &jobs), Err(ReporterSourceIssue::Identity));
            assert!(relay.admission_failure().is_none());
            scope.cancel();
            assert_eq!(relay.retry(&client, &scope, &jobs), Err(ReporterSourceIssue::Admission));
            assert!(
                matches!(relay.admission_failure(), Some(AdmissionError::ScopeClosed { scope_id }) if *scope_id == scope.id())
            );
            assert!(!relay.admission_retryable());
            assert!(matches!(relay.work, RelayWork::Rejected(AdmissionError::ScopeClosed { .. })));
            assert!(relay.retry_pending());
            assert!(matches!(relay.take_delivery()?, ReporterDelivery::Pending));
            assert_eq!(relay.original, Some(original));
            assert_eq!(relay.deadline(), Some(until));
            assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
            assert!(alias.loan()?.state()?.first.is_none());
        }
        Case::RetryExpired => {
            clock.0.store(until.as_nanos(), Ordering::Release);
            assert_eq!(relay.retry(&client, &scope, &jobs), Err(ReporterSourceIssue::Deadline));
            assert!(
                matches!(relay.admission_failure(), Some(AdmissionError::DeadlineExpired { deadline, now }) if *deadline == until && *now == until)
            );
            assert!(!relay.admission_retryable());
            assert!(matches!(
                relay.work,
                RelayWork::Rejected(AdmissionError::DeadlineExpired { .. })
            ));
            assert_eq!(relay.original, Some(original));
            assert_eq!(relay.deadline(), Some(until));
            assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
            assert!(alias.loan()?.state()?.first.is_none());
        }
        Case::Capacity => {
            let mut credits = Vec::with_capacity(owner.limits().terminal);
            for _ in 0..owner.limits().terminal {
                credits.push(hold_terminal_credit(&client, &wake, &scope, &jobs)?);
            }
            assert_eq!(relay.retry(&client, &scope, &jobs), Err(ReporterSourceIssue::Admission));
            assert!(matches!(relay.admission_failure(), Some(AdmissionError::CompletionSaturated)));
            assert!(relay.admission_retryable());
            assert!(matches!(relay.work, RelayWork::Idle));
            assert!(relay.retry_pending());
            assert_eq!(relay.original, Some(original));
            assert_eq!(relay.deadline(), Some(until));
            let mut released = credits.pop().ok_or("Original held completion missing")?;
            assert!(matches!(
                terminal(&client, &wake, &mut released)?.outcome,
                JobOutcome::Success(())
            ));
            clock.0.store(until.as_nanos(), Ordering::Release);
            assert_eq!(relay.retry(&client, &scope, &jobs), Err(ReporterSourceIssue::Deadline));
            assert!(matches!(relay.admission.as_ref(), Some(AdmissionError::CompletionSaturated)));
            assert!(
                matches!(relay.admission_failure(), Some(AdmissionError::DeadlineExpired { deadline, now }) if *deadline == until && *now == until)
            );
            assert!(!relay.admission_retryable());
            assert_eq!(relay.original, Some(original));
            assert_eq!(relay.deadline(), Some(until));
            assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
            for credit in &mut credits {
                assert!(matches!(
                    terminal(&client, &wake, credit)?.outcome,
                    JobOutcome::Success(())
                ));
            }
            assert!(alias.loan()?.state()?.first.is_none());
        }
        Case::Cancelled | Case::Expired | Case::Mismatched => {}
    }
    let retained = failure_custody::extract(case, &mut relay, until, expected)?;
    drop(holder);
    drop(blocker);
    drop(relay);
    let worker_retained = retained
        .as_ref()
        .is_some_and(|failure| matches!(failure, failure::ReporterSecondaryFailure::Worker { .. }));
    assert_eq!(
        scope.snapshot().reserved_bytes,
        CONTROLLER_BYTES + if worker_retained { worker::REPORTER_CORE_BYTES } else { 0 }
    );
    drop(retained);
    assert_eq!(scope.snapshot().reserved_bytes, CONTROLLER_BYTES);
    drop(alias);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(owner);
    Ok(())
}
#[test]
fn held_source_loan_defers_then_same_operation_retry_preserves_semantic_busy_failure() -> TestResult
{
    held_loan(Case::Deferred)
}
#[test]
fn queued_core_cancellation_retains_original_completion_without_touching_held_source() -> TestResult
{
    held_loan(Case::Cancelled)
}
#[test]
fn queued_core_deadline_equality_retains_original_timeout_without_touching_held_source()
-> TestResult {
    held_loan(Case::Expired)
}
#[test]
fn deferred_core_retry_rejects_foreign_scope_and_actual_closed_admission_without_new_credit()
-> TestResult {
    held_loan(Case::Rejected)
}

#[test]
fn deferred_core_held_loan_cutoff_and_post_return_admission_expiry_preserve_original_deadline()
-> TestResult {
    held_loan(Case::RetryExpired)
}
#[test]
fn deferred_core_capacity_then_expiry_preserves_both_actual_rejections_without_renewal()
-> TestResult {
    held_loan(Case::Capacity)
}

#[test]
fn mismatched_notice_retains_exact_actual_completion_and_refuses_failure_transfer() -> TestResult {
    held_loan(Case::Mismatched)
}
