// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise production dispatch, retained request identity and actual boundary retirement.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::{coordinator_entry::*, execution::*};
#[test]
fn lost_connection_settles_original_run_and_fresh_nonce_retry_never_readmits() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { connection, request } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("expected authenticated original".into());
    };
    assert_eq!(
        request.view()?.environment().units().collect::<Vec<_>>(),
        vec![65, 61, 0xd800, 0, 0]
    );
    session.control.close_connection(connection);
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:retired} if *retired==connection),
    )?;
    let id = request.request_id();
    drop(request);
    session.control.reply(
        connection,
        id,
        EntryOperationId::new(1)?,
        Moment::from_nanos(2_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    boundary.connect()?;
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:retired} if *retired!=connection),
    )?;
    let replies = lock(&boundary.replies);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].1, EntryOutcome::Accepted(run()?));
    assert_eq!(replies[0].0.request, id);
    assert_eq!(replies[0].0.nonce, [2; 32]);
    drop(replies);
    session.control.close();
    let completion = rig.completion(&mut session.completion)?;
    assert!(
        matches!(completion.outcome,JobOutcome::Success(report) if report.authenticated_launches==1)
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn immutable_frame_loan_retains_session_until_actual_native_close_and_return() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { request, .. } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("expected retained frame".into());
    };
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert_eq!(request.view()?.profile(), "profile_1");
    assert!(
        lock(&boundary.memory)
            .as_ref()
            .ok_or("missing real memory lease")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    drop(request);
    let completion = rig.completion(&mut session.completion)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_)));
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn client_initializing_reprobe_keeps_one_staged_request_and_original_counter() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    boundary.initializing.store(1, Ordering::Release);
    let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let input = CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?;
    let scope = native_scope(&rig)?;
    let mut session = crate::coordinator_entry::reserve_client(
        &rig.client.core,
        &scope,
        specification,
        input,
        Box::new(Lease(boundary.clone())),
    )?;
    fact(&rig, &session.control, |fact| {
        matches!(
            fact,
            EntryTransportObservation::DiscoveryVerified {
                phase: EntryDiscoveryPhase::Initializing,
                ..
            }
        )
    })?;
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    assert!(session.completion.try_take_completion().is_none());
    rig.clock.wait_until_registered(Moment::from_nanos(2_000_000_000))?;
    rig.clock.advance(2_000_000_000);
    let EntryTransportObservation::OutcomeReceived { request: observed, outcome, .. } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::OutcomeReceived { .. })
        })?
    else {
        return Err("expected retained request outcome".into());
    };
    assert_eq!(observed.get(), source()?.ticks() as u64);
    assert_eq!(outcome, EntryOutcome::Accepted(run()?));
    let completion = rig.completion(&mut session.completion)?;
    assert!(
        matches!(completion.outcome,JobOutcome::Success(report) if report.authenticated_launches==0 && report.retired_connections==2)
    );
    assert_eq!(boundary.writes.load(Ordering::Acquire), 3);
    Ok(())
}
#[test]
fn partial_prepare_failure_cannot_release_adopted_owner_before_checked_retirement() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    struct Release(std::sync::Arc<Boundary>);
    impl Drop for Release {
        fn drop(&mut self) {
            self.0.allow_retire.store(true, Ordering::Release);
            self.0.wake();
        }
    }
    let release = Release(boundary.clone());
    let (_scope, mut session) = server(&rig, &boundary)?;
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert!(
        lock(&boundary.memory)
            .as_ref()
            .ok_or("missing prepared memory")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    drop(release);
    let mut failures = Vec::new();
    let completion = loop {
        let observed = rig.wake.count();
        if let Some(completion) = session.completion.try_take_completion() {
            break completion;
        }
        if let Err(cause) = rig.client.drain_completions(NonZeroUsize::MIN) {
            failures.push(cause);
        }
        if let Some(completion) = session.completion.try_take_completion() {
            break completion;
        }
        rig.wake.wait_after(observed)?;
    };
    assert!(
        matches!(completion.outcome,JobOutcome::Failure(ref cause) if cause.native_cause().is_some_and(|cause|cause.stage()=="entry_partial_prepare"))
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    assert!(failures.len() <= 1);
    Ok(())
}
#[test]
fn expired_reconnect_cannot_relaunch_or_replace_late_actual_root_settlement() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { connection, request } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("expected original authenticated request".into());
    };
    let id = request.request_id();
    session.control.close_connection(connection);
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:closed} if *closed==connection),
    )?;
    drop(request);
    rig.clock.advance(31_000_000_000);
    session.control.reply(
        connection,
        id,
        EntryOperationId::new(1)?,
        Moment::from_nanos(2_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    boundary.connect()?;
    let failures = rejected_connection(&rig, &session.control, Some(connection))?;
    assert!(lock(&session.control.shared.state).requests.iter().all(Option::is_none));
    assert!(matches!(
        session.control.reply(
            connection,
            id,
            EntryOperationId::new(2)?,
            Moment::from_nanos(2_000_000_000),
            EntryOutcome::Rejected(EntryRejection::Capacity)
        ),
        Err(EntryTransportError::Authentication)
    ));
    assert!(lock(&boundary.replies).is_empty());
    assert!(!failures.is_empty());
    session.control.close();
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.authenticated_launches==1)
    );
    Ok(())
}
