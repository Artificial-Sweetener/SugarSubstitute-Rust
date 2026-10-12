// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual native custody after root shutdown and original private cleanup expiry.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult},
    scope::lock,
};
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobOutcome, JobPhase, Moment};
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
    let boundary = Boundary::new(&rig, true)?;
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let release = ReleaseBoundary(boundary.clone());
    let (_scope, mut session) = server(&rig, &boundary)?;
    boundary.retiring.wait()?;
    rig.clock.wait_until_registered(Moment::from_nanos(10_000_000_000))?;
    rig.clock.advance(9_000_000_000);
    session.control.close();
    rig.clock.advance(1_000_000_000);
    rig.clock.wait_until_registered(Moment::from_nanos(15_000_000_000))?;
    rig.clock.advance(5_000_000_000);
    let mut failures = Vec::new();
    loop {
        let observed = rig.wake.count();
        if let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
            failures.push(error);
        }
        let stopped = lock(&rig.client.core.state)
            .jobs
            .get(&session.control.shared.job_id)
            .and_then(|record| record.native_deadlines)
            .is_some_and(|slots| slots.stopped());
        if stopped {
            break;
        }
        rig.wake.wait_after(observed)?;
    }
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert!(
        lock(&boundary.memory)
            .as_ref()
            .ok_or("missing retained memory")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    session.control.close();
    drop(release);
    let completion = loop {
        let observed = rig.wake.count();
        if let Some(completion) = session.completion.try_take_completion() {
            break completion;
        }
        if let Err(error) = rig.client.drain_completions(NonZeroUsize::MIN) {
            failures.push(error);
        }
        if let Some(completion) = session.completion.try_take_completion() {
            break completion;
        }
        rig.wake.wait_after(observed)?;
    };
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
