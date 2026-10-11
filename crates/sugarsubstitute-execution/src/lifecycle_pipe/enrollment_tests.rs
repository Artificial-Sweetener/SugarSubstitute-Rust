// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real single-transfer suspended-desktop pin without native handles.
use super::super::enrollment::Enrollment;
use super::*;

fn key(run: RunKey) -> TestResult<ProcessKey> {
    Ok(ProcessKey { owner: run.owner, id: ProcessId::new(1)? })
}
#[test]
fn enrollment_rejects_foreign_run_and_owner_without_transferring_input() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    let mut enrollment = Enrollment::take(&session.control)?;
    assert_eq!(
        Enrollment::take(&session.control).err(),
        Some(LifecyclePipeError::AlreadyTransferred)
    );
    let run = run()?;
    let mut foreign = run;
    foreign.generation = RunGeneration::new(2)?;
    let result = enrollment.bind(foreign, key(run)?, 17, |_| PeerInput::Simulated);
    assert_eq!(result, Err((LifecyclePipeError::Closed, 17)));
    let mut process = key(run)?;
    process.owner.epoch = CoordinatorEpoch::new([3; 16])?;
    assert_eq!(
        enrollment.bind(run, process, 19, |_| PeerInput::Simulated),
        Err((LifecyclePipeError::Closed, 19))
    );
    enrollment.bind(run, key(run)?, (), |_| PeerInput::Simulated).map_err(|(error, _)| error)?;
    drop(enrollment);
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::Connected))?;
    assert_eq!(boundary.adopted.load(Ordering::Acquire), 1);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    drop(session);
    drop(scope);
    drop(rig.owner);
    Ok(())
}
#[test]
fn cancelled_enrollment_retains_untransferred_input_and_pin_until_drop() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    let mut enrollment = Enrollment::take(&session.control)?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    let run = run()?;
    assert_eq!(
        enrollment.bind(run, key(run)?, 23, |_| PeerInput::Simulated),
        Err((LifecyclePipeError::Closed, 23))
    );
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.adopted.load(Ordering::Acquire), 0);
    drop(enrollment);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    drop(session);
    drop(scope);
    drop(rig.owner);
    Ok(())
}

#[test]
fn same_run_different_sessions_cannot_mix_bootstrap_and_enrollment() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut first = session(&rig, &scope, &boundary, true)?;
    let second_boundary = Boundary::new();
    let mut specification = spec(2, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let mut second = reserve(
        &rig.client.core,
        &scope,
        specification,
        Some(run()?),
        Moment::from_nanos(1000),
        Box::new(Lease(second_boundary.clone())),
    )?;
    let enrollment = Enrollment::take(&first.control)?;
    fact(&rig, &first.control, |f| matches!(f, LifecyclePipeObservation::Prepared))?;
    fact(&rig, &second.control, |f| matches!(f, LifecyclePipeObservation::Prepared))?;
    let first_bootstrap = first.control.take_bootstrap()?.ok_or("Missing first bootstrap")?;
    let second_bootstrap = second.control.take_bootstrap()?.ok_or("Missing second bootstrap")?;
    assert!(enrollment.matches(&first_bootstrap.shared));
    assert!(!enrollment.matches(&second_bootstrap.shared));
    assert_eq!(first.control.prepared_run(), Some(run()?));
    drop(first_bootstrap);
    drop(second_bootstrap);
    drop(enrollment);
    second.control.close();
    assert!(matches!(rig.completion(&mut first.completion)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(rig.completion(&mut second.completion)?.outcome, JobOutcome::Cancelled));
    drop(first);
    drop(second);
    drop(scope);
    drop(rig.owner);
    Ok(())
}
