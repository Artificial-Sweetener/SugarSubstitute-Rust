// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original failures, command deadlines and caller-owned allocations at fault boundaries.
use super::*;
pub(super) fn completion(
    rig: &Rig,
    session: &mut LifecyclePipeSession,
) -> TestResult<JobCompletion<LifecyclePipeReport, LifecyclePipeFailure>> {
    loop {
        let observed = rig.wake.count();
        if let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
            assert!(matches!(
                error.operation(),
                Operation::NativeLifecycle | Operation::Infrastructure
            ));
            // Failure delivery precedes terminal; drain the still-pending activity batch
            // before waiting for the next coalesced wake.
            continue;
        }
        if let Some(completion) = session.completion.try_take_completion() {
            return Ok(completion);
        }
        rig.wake.wait_after(observed)?;
    }
}
#[test]
fn fresh_deadline_after_native_read_prevents_later_write_effect_and_success_publication()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.hold_read.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(400))?;
    boundary.reading.wait()?;
    let mut output = session.control.take_write_buffer()?.ok_or("Missing output")?;
    output.as_mut_bytes()[0] = 1;
    session
        .control
        .write(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900), output, 1)
        .map_err(|e| e.error)?;
    rig.clock.advance(400);
    boundary.read_release.open();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(session.control.take_observation().is_none());
    Ok(())
}
#[test]
fn rejected_encoded_buffer_keeps_original_bytes_claim_and_operation_identity() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let mut output = session.control.take_write_buffer()?.ok_or("Missing output")?;
    output.as_mut_bytes()[..6].copy_from_slice(b"Secret");
    let mut rejected = session
        .control
        .write(
            LifecyclePipeOperationId::new(1)?,
            Moment::from_nanos(900),
            output,
            MAX_LIFECYCLE_MESSAGE_BYTES + 1,
        )
        .err()
        .ok_or("Expected bound rejection")?;
    assert_eq!(rejected.error, LifecyclePipeError::InvalidMessage);
    assert_eq!(&rejected.buffer.as_mut_bytes()[..6], b"Secret");
    assert!(!format!("{rejected:?}").contains("Secret"));
    session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), rejected.buffer, 6)
        .map_err(|e| e.error)?;
    fact(&rig, &session.control, |f| {
        matches!(f, LifecyclePipeObservation::Written { bytes: 6, .. })
    })?;
    session.control.close();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn output_guard_blocks_terminal_until_storage_returns_after_actual_native_close() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing output")?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert!(scope.snapshot().reserved_bytes >= MIN_LIFECYCLE_PIPE_BYTES);
    drop(output);
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn original_native_failure_then_retirement_panic_preserves_fatal_flag_and_first_cause() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.panic_retire.store(true, Ordering::Release);
    boundary.retire_failures.store(1, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    let JobOutcome::Failure(error) = completion(&rig, &mut session)?.outcome else {
        return Err("Expected retained failure".into());
    };
    assert_eq!(error.native_cause().stage(), "partial_pipe_creation");
    assert!(error.contains_owner_panic());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert!(!format!("{error:?}").contains("Secret"));
    Ok(())
}
#[test]
fn panic_before_native_failure_remains_distinct_typed_terminal_after_actual_retirement()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.panic_prepare.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Panicked(_)));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn admission_rejection_has_no_native_effect_or_identity_consumption() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut too_small = spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES - 1)?;
    too_small.operation = Operation::NativeLifecycle;
    assert!(matches!(
        reserve(
            &rig.client.core,
            &scope,
            too_small,
            Some(run()?),
            Moment::from_nanos(1000),
            Box::new(Lease(boundary.clone()))
        ),
        Err(AdmissionError::NativeReservationTooSmall { .. })
    ));
    assert!(matches!(
        reserve(
            &rig.client.core,
            &scope,
            spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?,
            Some(run()?),
            Moment::from_nanos(0),
            Box::new(Lease(boundary.clone()))
        ),
        Err(AdmissionError::DeadlineExpired { .. })
    ));
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(boundary.prepares.load(Ordering::Acquire), 0);
    let mut accepted = session(&rig, &scope, &boundary, true)?;
    accepted.control.close();
    assert!(matches!(completion(&rig, &mut accepted)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn bounded_failure_inventory_preserves_original_panic_and_fatal_identity_flags() -> TestResult {
    let mut outcome = Some(JobOutcome::Failure(failure("first_native")));
    for _ in 0..10 {
        control::retain_failure(&mut outcome, failure("secondary"));
    }
    let mut fatal = failure("late_fatal");
    fatal.identity_unrestored = true;
    fatal.cause.retain_panic(&PanicCause::new("Secret panic".to_owned()));
    control::retain_failure(&mut outcome, fatal);
    let Some(JobOutcome::Failure(error)) = outcome else {
        return Err("Missing aggregated failure".into());
    };
    assert_eq!(error.native_cause().stage(), "first_native");
    assert!(error.requires_process_shutdown());
    assert!(error.contains_owner_panic());
    assert!(error.clone().contains_owner_panic());
    assert!(!format!("{error:?}").contains("Secret"));
    Ok(())
}

#[test]
fn native_issue_panic_retains_read_and_write_allocations_until_actual_retirement() -> TestResult {
    for write in [false, true] {
        let rig = Rig::new()?;
        let scope = scope(&rig)?;
        let boundary = Boundary::new();
        boundary.allow_retire.store(false, Ordering::Release);
        if write {
            boundary.panic_write.store(true, Ordering::Release);
        } else {
            boundary.panic_read.store(true, Ordering::Release);
        }
        let mut session = session(&rig, &scope, &boundary, true)?;
        connect(&rig, &session)?;
        if write {
            let mut output = session.control.take_write_buffer()?.ok_or("Missing output")?;
            output.as_mut_bytes()[0] = 1;
            session
                .control
                .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
                .map_err(|e| e.error)?;
        } else {
            session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
        }
        boundary.retiring.wait()?;
        assert!(session.completion.try_take_completion().is_none());
        assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
        let memory = lock(&boundary.memory).take().ok_or("Missing admitted memory")?;
        // All three claims still occupy exactly192KiB; a freed issue buffer would allow this.
        assert!(memory.claim(1).is_err());
        boundary.allow_retire.store(true, Ordering::Release);
        boundary.wake(if write { Role::Write } else { Role::Read })?;
        assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Panicked(_)));
        assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
        drop(memory);
    }
    Ok(())
}

#[test]
fn terminal_retains_passive_actual_write_and_pending_read_observations() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let mut output = session.control.take_write_buffer()?.ok_or("Missing output")?;
    output.as_mut_bytes()[0] = 1;
    session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
        .map_err(|e| e.error)?;
    session.control.read(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900))?;
    loop {
        let observed = rig.wake.count();
        let ready = || {
            let state = lock(&session.control.shared.state);
            state.observations.iter().any(|f| matches!(f, LifecyclePipeObservation::Written { operation, bytes: 1 } if operation.get() == 1))
                && state.observations.iter().any(|f| matches!(f, LifecyclePipeObservation::ReadPending { operation } if operation.get() == 2))
        };
        if ready() {
            break;
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if ready() {
            break;
        }
        rig.wake.wait_after(observed)?;
    }
    session.control.close();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    let mut write = false;
    let mut pending = false;
    while let Some(fact) = session.control.take_observation() {
        match fact {
            LifecyclePipeObservation::Written { operation, bytes: 1 } if operation.get() == 1 => {
                write = true
            }
            LifecyclePipeObservation::ReadPending { operation } if operation.get() == 2 => {
                pending = true
            }
            _ => return Err("Unexpected retained fact".into()),
        }
    }
    assert!(write && pending);
    Ok(())
}

#[test]
fn concurrent_write_retirement_preserves_exact_effect_without_rewriting_read_failure() -> TestResult
{
    for actual in [Some(4), Some(3), None] {
        let rig = Rig::new()?;
        let scope = scope(&rig)?;
        let boundary = Boundary::new();
        boundary.pending_write.store(true, Ordering::Release);
        let mut session = session(&rig, &scope, &boundary, true)?;
        connect(&rig, &session)?;
        let mut output = session.control.take_write_buffer()?.ok_or("Missing output")?;
        output.as_mut_bytes()[..4].copy_from_slice(b"Ping");
        session
            .control
            .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 4)
            .map_err(|e| e.error)?;
        boundary.writing.wait()?;
        // Model the external kernel's completed, partial or aborted write. Read fails
        // first in the same protocol pass, before that write completion can be polled.
        *lock(&boundary.retired_write) = actual;
        boundary.fail_read.store(true, Ordering::Release);
        session.control.read(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900))?;
        boundary.wake(Role::Write)?;
        let JobOutcome::Failure(error) = completion(&rig, &mut session)?.outcome else {
            return Err("Expected original read failure".into());
        };
        assert_eq!(error.native_cause().stage(), "concurrent_read_failure");
        assert_eq!(boundary.write_polls.load(Ordering::Acquire), 0);
        assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
        if actual == Some(4) {
            assert!(
                matches!(session.control.take_observation(), Some(LifecyclePipeObservation::Written { operation, bytes: 4 }) if operation.get() == 1)
            );
        } else {
            assert!(session.control.take_observation().is_none());
        }
    }
    Ok(())
}
