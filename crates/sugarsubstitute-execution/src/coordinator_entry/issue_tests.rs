// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Delay only external operation preparation and verify actual owner issue fences.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    native_deadlines::Role,
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobOutcome, Operation};
fn fenced(role: Role, close: bool) -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let release = ReleaseBoundary(boundary.clone());
    if role == Role::Handshake {
        *lock(&boundary.pause_role) = Some(role);
    }
    let (_scope, mut session) = server(&rig, &boundary)?;
    if role == Role::Handshake {
        fact(&rig, &session.control, |fact| matches!(fact, EntryTransportObservation::Prepared))?;
    } else {
        activate(&rig, &session)?;
        *lock(&boundary.pause_role) = Some(role);
        boundary.connect()?;
    }
    boundary.issue_prepared.wait()?;
    if close {
        session.control.close();
    } else {
        rig.clock.advance(if role == Role::Handshake { 30_000_000_000 } else { 2_000_000_000 });
    }
    boundary.issue_release.open();
    if !close && role != Role::Handshake {
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
        })?;
        session.control.close();
    }
    let completion = rig.completion(&mut session.completion)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_) | JobOutcome::TimedOut));
    let issued = match role {
        Role::Handshake => &boundary.connects_issued,
        Role::Read => &boundary.reads_issued,
        Role::Write => &boundary.writes,
    };
    assert_eq!(issued.load(Ordering::Acquire), 0);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    drop(release);
    Ok(())
}
#[test]
fn close_during_connect_preparation_fences_native_acceptance() -> TestResult {
    fenced(Role::Handshake, true)
}
#[test]
fn original_preparation_expiry_fences_pending_server_acceptance() -> TestResult {
    fenced(Role::Handshake, false)
}
#[test]
fn close_during_read_preparation_fences_native_acceptance() -> TestResult {
    fenced(Role::Read, true)
}
#[test]
fn original_read_probe_expiry_fences_native_acceptance() -> TestResult {
    fenced(Role::Read, false)
}
#[test]
fn close_during_write_preparation_fences_native_acceptance() -> TestResult {
    fenced(Role::Write, true)
}
#[test]
fn original_write_probe_expiry_fences_native_acceptance() -> TestResult {
    fenced(Role::Write, false)
}
fn client_fenced(during_prepare: bool, close: bool) -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    let release = ReleaseBoundary(boundary.clone());
    boundary.pause_prepare.store(during_prepare, Ordering::Release);
    *lock(&boundary.pause_role) = Some(Role::Handshake);
    let scope = native_scope(&rig)?;
    let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let input = CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?;
    let mut session = reserve_client(
        &rig.client.core,
        &scope,
        specification,
        input,
        Box::new(Lease(boundary.clone())),
    )?;
    boundary.issue_prepared.wait()?;
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 0);
    if close {
        session.control.close();
    } else {
        rig.clock.advance(30_000_000_000);
    }
    boundary.issue_release.open();
    let completion = rig.completion(&mut session.completion)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_) | JobOutcome::TimedOut));
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 0);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    drop(release);
    Ok(())
}
#[test]
fn close_during_client_owner_preparation_fences_actual_connect() -> TestResult {
    client_fenced(true, true)
}
#[test]
fn original_client_expiry_during_owner_preparation_fences_actual_connect() -> TestResult {
    client_fenced(true, false)
}
#[test]
fn close_during_client_connect_preparation_fences_actual_connect() -> TestResult {
    client_fenced(false, true)
}
#[test]
fn original_client_expiry_during_connect_preparation_fences_actual_connect() -> TestResult {
    client_fenced(false, false)
}
#[test]
fn client_open_and_message_mode_failures_preserve_original_cause_until_actual_retirement()
-> TestResult {
    for stage in ["entry_open_client", "entry_client_message_mode"] {
        let rig = Rig::new()?;
        let boundary = Boundary::new(&rig, false)?;
        boundary.allow_retire.store(false, Ordering::Release);
        *lock(&boundary.connect_failure) = Some(stage);
        let release = ReleaseBoundary(boundary.clone());
        let scope = native_scope(&rig)?;
        let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
        specification.operation = Operation::NativeLifecycle;
        let input = CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?;
        let mut session = reserve_client(
            &rig.client.core,
            &scope,
            specification,
            input,
            Box::new(Lease(boundary.clone())),
        )?;
        boundary.retiring.wait()?;
        assert!(session.completion.try_take_completion().is_none());
        assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
        assert!(
            lock(&boundary.memory)
                .as_ref()
                .ok_or("missing retained memory")?
                .claim(MIN_ENTRY_CLIENT_BYTES)
                .is_err()
        );
        drop(release);
        let mut failures = Vec::new();
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
        assert!(matches!(completion.outcome, JobOutcome::Failure(ref error)
            if error.native_cause().is_some_and(|cause| cause.stage()==stage)));
        assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 1);
        assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
        assert!(!failures.is_empty());
    }
    Ok(())
}
