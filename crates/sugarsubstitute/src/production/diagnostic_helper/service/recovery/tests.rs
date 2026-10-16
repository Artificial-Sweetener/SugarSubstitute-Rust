// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise original execution delivery and checked storage retirement before reusing the seed cell.
use super::*;
use crate::production::diagnostic_helper::tests::{
    TestClock, TestError, Wake, key, observe, prepared, seed, spec,
};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_supervision::diagnostic_storage::fixture::{
    DiagnosticFixtureFailure, DiagnosticStorageFixture,
};

fn delivered(
    service: &mut SeedService,
    client: &ExecutionClient,
    wake: &Wake,
    cleanup: bool,
) -> Result<(), TestError> {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if cleanup {
            service.capture_cleanup(&batch)?;
        } else {
            service.capture(&batch)?;
        }
        if (cleanup && service.cleanup_completion.is_some())
            || (!cleanup && service.outcome().is_some())
        {
            return Ok(());
        }
        if batch.remaining > 0 {
            continue;
        }
        let (guard, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |value| *value == before,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(guard);
        if timeout.timed_out() {
            return Err("Actual seed retirement delivery stalled".into());
        }
    }
}
fn exercise(started: bool, cancelled: bool) -> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut construction = spec(jobs.allocate()?);
    construction.reservation_bytes = crate::production::incident_host::CONTROLLER_BYTES;
    let mut constructor =
        client.submit_blocking(&scope, construction, |context| SeedService::new(&context))?;
    let completion = observe(&client, &wake, &mut constructor)?;
    let mut service = match completion.outcome {
        JobOutcome::Success(service) => service,
        _ => return Err("Actual seed source construction failed".into()),
    };
    drop(completion.memory);
    drop(constructor);
    let actual = seed()?;
    service.input_identity =
        Some(SeedIdentity { operation: key()?, key: actual.key(), revision: actual.revision() });
    service.installed = true;
    let original = spec(jobs.allocate()?);
    service.requested =
        Some((scope.owner_id(), scope.id(), original.deadline.ok_or("Missing cutoff")?));
    if started {
        let source = service.source.alias();
        let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
        let worker_gate = gate.clone();
        service.job = Some(client.submit_blocking(&scope, original, move |context| {
            let mut loan = source.loan()?;
            let fixture =
                DiagnosticStorageFixture::new().map_err(|_| SeedServiceIssue::Identity)?;
            *loan.operation()? =
                prepared(&context, &fixture).map_err(|_| SeedServiceIssue::Identity)?;
            fixture.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
            let failure = loan
                .operation()?
                .publish(actual, &context)
                .err()
                .ok_or(SeedServiceIssue::Identity)?;
            loan.operation()?.first = Some(failure);
            if cancelled {
                let mut state =
                    worker_gate.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                state.0 = true;
                worker_gate.1.notify_all();
                let (state, timeout) = worker_gate
                    .1
                    .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                drop(state);
                if timeout.timed_out() {
                    return Err(SeedServiceIssue::Busy);
                }
            }
            Ok::<_, SeedServiceIssue>(())
        })?);
        if cancelled {
            let state = gate.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let (mut state, timeout) = gate
                .1
                .wait_timeout_while(state, Duration::from_secs(10), |state| !state.0)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if timeout.timed_out() {
                return Err("Actual seed worker never reached cancellation boundary".into());
            }
            service.cancel();
            state.1 = true;
            gate.1.notify_all();
        }
        delivered(&mut service, &client, &wake, false)?;
        assert_eq!(
            service.outcome(),
            Some(if cancelled { TerminalKind::Cancelled } else { TerminalKind::Success })
        );
    }
    assert!(!service.failed_retirement_matches(key()?, seed()?.key(), seed()?.revision()));
    assert!(service.prepare_failed_retirement().is_err());
    service.submit_cleanup(&client, &scope, &jobs, Moment::from_nanos(100))?;
    delivered(&mut service, &client, &wake, true)?;
    assert!(service.files_retired());
    assert!(!service.publication_succeeded());
    assert!(service.failed_retirement_matches(key()?, seed()?.key(), seed()?.revision()));
    service.prepare_failed_retirement()?;
    let original_cause = std::ptr::from_ref(service.failure().ok_or("Actual failure missing")?);
    service.prepare_failed_retirement()?;
    assert!(std::ptr::eq(
        original_cause,
        std::ptr::from_ref(service.failure().ok_or("Actual failure changed")?)
    ));
    let retired = service.take_failed_retirement()?;
    assert!(!retired.owner_failed());
    assert!(retired.failure().is_some());
    if started {
        assert!(retired.failure().and_then(Error::source).is_some());
    }
    assert!(service.outcome().is_none());
    assert!(!service.installed);
    assert!(service.input_identity.is_none());
    assert!(!service.files_retired());
    assert!(service.source.inspect(|_| ()).is_none());
    let before = scope.snapshot().reserved_bytes;
    drop(retired);
    let released = if started { DIAGNOSTIC_STORAGE_BYTES + 32 * 1024 } else { 32 * 1024 };
    assert_eq!(scope.snapshot().reserved_bytes, before - released);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn logical_seed_failure_retains_original_cause_after_checked_source_reuse() -> Result<(), TestError>
{
    exercise(true, false)
}
#[test]
fn never_executed_seed_retires_under_actual_cleanup_without_a_fabricated_worker_terminal()
-> Result<(), TestError> {
    exercise(false, false)
}

#[test]
fn cancelled_seed_preserves_partial_storage_cause_and_original_cancelled_terminal()
-> Result<(), TestError> {
    exercise(true, true)
}
