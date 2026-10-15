// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual native custody after root shutdown and original private cleanup expiry.
use super::kernel::*;
use crate::{
    admission::Core,
    coordinator_entry::*,
    native::NativeWork,
    runtime_test_support::{ClockPort, Rig, TestResult, spec},
    scope::lock,
    task::Work,
};
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::execution::{
    ExecutionId, ExecutionPort, JobClass, JobOutcome, JobPhase, Moment, Operation, ScopeId,
    ScopeSpec,
};
#[test]
fn serving_past_preparation_has_no_private_cleanup_budget_and_retains_native_owner() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    boundary.allow_retire.store(false, Ordering::Release);
    let release = ReleaseBoundary(boundary.clone());
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    rig.clock.advance(40_000_000_000);
    session.control.close();
    boundary.retiring.wait()?;
    let state = lock(&rig.client.core.state);
    let deadlines = state
        .jobs
        .get(&session.control.shared.job_id)
        .and_then(|record| record.native_deadlines)
        .ok_or("missing actual deadline owner")?;
    assert!(deadlines.stopped());
    drop(state);
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert!(
        lock(&boundary.memory)
            .as_ref()
            .ok_or("missing memory lease")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    drop(release);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
#[test]
fn first_private_cleanup_deadlines_do_not_renew_and_expiry_retains_partial_native_owner()
-> TestResult {
    let rig = Rig::new()?;
    // Drive the real admitted native owner directly: mailbox error wakes precede private timer updates.
    let core =
        Core::new(ExecutionId::new(99)?, Arc::new(ClockPort(rig.clock.clone())), rig.wake.clone());
    let scope =
        core.create_scope_kind(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None, true)?;
    let boundary = Boundary::new(&rig, true)?;
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let release = ReleaseBoundary(boundary.clone());
    let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_SERVER_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let mut session = reserve_server(
        &core,
        &scope,
        specification,
        CoordinatorEntryServerInput::new(facts()?)?,
        Moment::from_nanos(30_000_000_000),
        Box::new(Lease(boundary.clone())),
    )?;
    let (queued, _) =
        core.take_native_at(Moment::from_nanos(0)).ok_or("Actual native dequeue missing")?;
    let Work::Native(NativeWork::CoordinatorEntry(work)) = queued.work else {
        return Err("Wrong actual native owner selected".into());
    };
    let mut active = worker::Active::new(work, queued.context, &core, Moment::from_nanos(0));
    active.progress(&core, Moment::from_nanos(0), true);
    assert!(
        lock(&core.state)
            .jobs
            .get(&session.control.shared.job_id)
            .and_then(|record| record.native_deadlines)
            .is_some_and(
                |slots| slots.handshake_deadline() == Some(Moment::from_nanos(10_000_000_000))
            )
    );
    rig.clock.advance(9_000_000_000);
    session.control.close();
    active.progress(&core, Moment::from_nanos(9_000_000_000), true);
    assert!(
        lock(&core.state)
            .jobs
            .get(&session.control.shared.job_id)
            .and_then(|record| record.native_deadlines)
            .is_some_and(
                |slots| slots.handshake_deadline() == Some(Moment::from_nanos(10_000_000_000))
            )
    );
    rig.clock.advance(1_000_000_000);
    active.progress(&core, Moment::from_nanos(10_000_000_000), true);
    assert!(
        lock(&core.state)
            .jobs
            .get(&session.control.shared.job_id)
            .and_then(|record| record.native_deadlines)
            .is_some_and(
                |slots| slots.handshake_deadline() == Some(Moment::from_nanos(15_000_000_000))
            )
    );
    rig.clock.advance(5_000_000_000);
    active.progress(&core, Moment::from_nanos(15_000_000_000), true);
    assert!(
        lock(&core.state)
            .jobs
            .get(&session.control.shared.job_id)
            .and_then(|record| record.native_deadlines)
            .is_some_and(|slots| slots.stopped())
    );
    assert!(!active.completed());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert!(
        lock(&boundary.memory)
            .as_ref()
            .ok_or("missing retained memory")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    assert_eq!(scope.snapshot().reserved_bytes, MIN_ENTRY_SERVER_BYTES);
    let mut failures = Vec::new();
    while let Err(error) = core.mailbox.drain(NonZeroUsize::MIN) {
        failures.push(error);
    }
    session.control.close();
    active.progress(&core, Moment::from_nanos(15_000_000_000), true);
    assert!(!active.completed());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    drop(release);
    active.progress(&core, Moment::from_nanos(15_000_000_000), true);
    assert!(active.completed());
    let completion = session.completion.try_take_completion().ok_or("Actual terminal missing")?;
    assert!(
        matches!(completion.outcome,JobOutcome::Failure(ref error) if error.native_cause().is_some_and(|cause|cause.stage()=="entry_partial_prepare"))
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    assert!(!failures.is_empty());
    Ok(())
}
#[test]
fn actual_boundary_panic_and_retirement_failure_preserve_authoritative_panic_fact() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    boundary.panic_prepare.store(true, Ordering::Release);
    boundary.fail_retire_once.store(true, Ordering::Release);
    let (_scope, mut session) = server(&rig, &boundary)?;
    let mut failures = Vec::new();
    loop {
        let observed = rig.wake.count();
        if matches!(session.completion.status().phase, JobPhase::Terminal(_)) {
            break;
        }
        while let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
            failures.push(error);
        }
        if matches!(session.completion.status().phase, JobPhase::Terminal(_)) {
            break;
        }
        rig.wake.wait_after(observed)?;
    }
    let completion = loop {
        let observed = rig.wake.count();
        while let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
            failures.push(error);
        }
        if let Some(completion) = session.completion.try_take_completion() {
            break completion;
        }
        rig.wake.wait_after(observed)?;
    };
    if let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
        failures.push(error);
    }
    assert!(
        matches!(completion.outcome,JobOutcome::Failure(ref error) if error.contains_owner_panic() && error.native_cause().is_some_and(|cause|cause.stage()=="entry_partial_retirement"))
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    assert!(!failures.is_empty());
    Ok(())
}
