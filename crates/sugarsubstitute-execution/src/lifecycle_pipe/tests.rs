// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive real admitted transport ownership with controlled external pipe effects.
use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{execution::*, lifecycle::*};

#[path = "enrollment_tests.rs"]
mod enrollment;
#[path = "test_support.rs"]
mod kernel;
use kernel::{Boundary, Lease};
#[path = "dispatch_tests.rs"]
mod dispatch;
#[path = "dispatch_clock_tests.rs"]
mod dispatch_clock;
#[path = "dispatch_custody_tests.rs"]
mod dispatch_custody;
#[path = "failure_tests.rs"]
mod failures;
#[path = "identity_failure_tests.rs"]
mod identity_failures;
#[path = "late_input_tests.rs"]
mod late_input;
fn run() -> TestResult<RunKey> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}
fn scope(rig: &Rig) -> TestResult<Scope> {
    Ok(rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?)
}
fn session(
    rig: &Rig,
    scope: &Scope,
    boundary: &Arc<Boundary>,
    server: bool,
) -> TestResult<LifecyclePipeSession> {
    let mut specification = spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    Ok(reserve(
        &rig.client.core,
        scope,
        specification,
        if server { Some(run()?) } else { None },
        Moment::from_nanos(1000),
        Box::new(Lease(boundary.clone())),
    )?)
}
fn fact(
    rig: &Rig,
    control: &LifecyclePipeControl,
    select: impl Fn(&LifecyclePipeObservation) -> bool,
) -> TestResult<LifecyclePipeObservation> {
    loop {
        let observed = rig.wake.count();
        while let Some(fact) = control.take_observation() {
            if select(&fact) {
                return Ok(fact);
            }
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(fact) = control.take_observation()
            && select(&fact)
        {
            return Ok(fact);
        }
        rig.wake.wait_after(observed)?;
    }
}
fn connect(rig: &Rig, session: &LifecyclePipeSession) -> TestResult {
    session.control.bind_input(PeerInput::Simulated);
    fact(rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Connected))?;
    Ok(())
}

#[test]
fn authenticated_session_outlives_handshake_and_held_frame_preserves_admitted_storage() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    let received =
        fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Received { .. }))?;
    let LifecyclePipeObservation::Received { frame, .. } = received else {
        return Err("Wrong observation".into());
    };
    assert_eq!(frame.as_bytes(), b"Hello");
    assert_eq!(
        session.control.read(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900)),
        Err(LifecyclePipeError::Busy)
    );
    session.control.verify_capability([7; 32], Moment::from_nanos(1000))?;
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Authenticated(_)))?;
    assert_eq!(session.control.original_deadline(), Moment::from_nanos(1000));
    rig.clock.advance(1001);
    assert_eq!(session.control.original_deadline(), Moment::from_nanos(1000));
    let mut output = session.control.take_write_buffer()?.ok_or("Missing output storage")?;
    output.as_mut_bytes()[..4].copy_from_slice(b"Ping");
    session
        .control
        .write(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(2000), output, 4)
        .map_err(|e| e.error)?;
    fact(&rig, &session.control, |f| {
        matches!(f, LifecyclePipeObservation::Written { bytes: 4, .. })
    })?;
    assert_eq!(
        session.control.read(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(2000)),
        Err(LifecyclePipeError::InvalidOperation)
    );
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    drop(frame);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    drop(session);
    drop(scope);
    drop(rig.owner);
    Ok(())
}

#[test]
fn pending_read_deadline_wakes_parked_native_owner_without_native_callback() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(400))?;
    boundary.reading.wait()?;
    let pending = fact(&rig, &session.control, |f| {
        matches!(f, LifecyclePipeObservation::ReadPending { .. })
    })?;
    assert!(
        matches!(pending, LifecyclePipeObservation::ReadPending { operation } if operation.get() == 1)
    );
    rig.clock.wait_until_registered(Moment::from_nanos(400))?;
    rig.clock.advance(400);
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Read)?;
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(rig.clock.expired_sleep_calls.load(Ordering::Acquire), 0);
    drop(session);
    drop(scope);
    drop(rig.owner);
    Ok(())
}

#[test]
fn cancelled_creation_waits_for_input_pin_and_adopts_late_input_before_terminal() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    session.control.bind_input(PeerInput::Simulated);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.prepares.load(Ordering::Acquire), 0);
    assert_eq!(boundary.adopted.load(Ordering::Acquire), 1);
    drop(session);
    drop(scope);
    drop(rig.owner);
    Ok(())
}

#[test]
fn cancellation_preserves_launch_pin_until_checked_owner_retirement() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Prepared))?;
    let pin = session.control.take_bootstrap()?.ok_or("Missing bootstrap")?;
    assert_eq!(pin.handle(), 19);
    assert_eq!(
        session.control.take_bootstrap().err(),
        Some(LifecyclePipeError::AlreadyTransferred)
    );
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    drop(pin);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    drop(session);
    drop(scope);
    drop(rig.owner);
    Ok(())
}

#[test]
fn partial_creation_failure_retains_first_cause_through_failed_close_and_actual_retry() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.retire_failures.store(1, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    boundary.retiring.wait()?;
    rig.client.core.changed();
    let completion = loop {
        let observed = rig.wake.count();
        if let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
            assert_eq!(error.operation(), Operation::NativeLifecycle);
        }
        if let Some(completion) = session.completion.try_take_completion() {
            break completion;
        }
        rig.wake.wait_after(observed)?;
    };
    let JobOutcome::Failure(error) = completion.outcome else {
        return Err("Expected failure".into());
    };
    assert_eq!(error.native_cause().stage(), "partial_pipe_creation");
    assert!(!error.contains_owner_panic());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    drop(session);
    drop(scope);
    drop(rig.owner);
    Ok(())
}
