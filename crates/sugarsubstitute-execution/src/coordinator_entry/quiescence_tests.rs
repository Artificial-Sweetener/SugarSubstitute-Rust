// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real server fence against queued routes, live peers and checked-retired frame custody.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult},
    scope::lock,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::{coordinator_entry::*, execution::*};

#[test]
fn normal_idle_fence_retains_actual_retired_launch_loan_until_original_owner_returns_it()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    boundary.hold_peer_close.store(true, Ordering::Release);
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { connection, request } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("Actual request missing".into());
    };
    let original = request.until();
    assert!(!session.control.try_close_when_idle()?);
    assert!(!lock(&session.control.shared.state).stopped);
    session.control.reply(
        connection,
        request.request_id(),
        EntryOperationId::new(1)?,
        Moment::from_nanos(1_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    fact(&rig, &session.control, |fact| matches!(fact, EntryTransportObservation::Written { .. }))?;
    boundary.peer_retirement_issued.wait()?;
    assert!(!session.control.try_close_when_idle()?);
    assert_eq!(boundary.native_closes.load(Ordering::Acquire), 0);
    boundary.close_peer(0)?;
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual==connection),
    )?;
    {
        let state = lock(&session.control.shared.state);
        assert!(state.requests.iter().all(Option::is_none));
        let slot = state.connections.iter().find(|slot| slot.loan).ok_or("Actual loan lost")?;
        assert!(slot.id.is_none() && slot.peer.is_none() && slot.header.is_none());
    }
    assert!(session.control.try_close_when_idle()?);
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(request.until(), original);
    assert_eq!(request.view()?.profile(), "profile_1");
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    drop(request);
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.authenticated_launches==1 && report.retired_connections==1)
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn normal_idle_fence_rejects_queued_authenticated_launch_without_taking_or_abandoning_it()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    boundary.connect()?;
    loop {
        let observed = rig.wake.count();
        // Drain the real wake owner without consuming its queued authenticated frame.
        rig.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        let ready = lock(&session.control.shared.state).observations.iter().any(|envelope| {
            matches!(envelope.observation, EntryTransportObservation::RequestAuthenticated { .. })
        });
        if ready {
            break;
        }
        rig.wake.wait_after(observed)?;
    }
    assert!(!session.control.try_close_when_idle()?);
    assert!(!lock(&session.control.shared.state).stopped);
    let EntryTransportObservation::RequestAuthenticated { connection, request } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("Queued original launch lost".into());
    };
    assert!(!session.control.try_close_when_idle()?);
    session.control.reply(
        connection,
        request.request_id(),
        EntryOperationId::new(1)?,
        Moment::from_nanos(1_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    session.control.close();
    drop(request);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
#[test]
fn normal_idle_fence_samples_no_clock_and_rejects_pending_activation() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let release = ReleaseBoundary(boundary.clone());
    *lock(&boundary.pause_role) = Some(crate::native_deadlines::Role::Handshake);
    let (_scope, mut session) = server(&rig, &boundary)?;
    fact(&rig, &session.control, |fact| matches!(fact, EntryTransportObservation::Prepared))?;
    boundary.issue_prepared.wait()?;
    session.control.activate_server(owner()?, Moment::from_nanos(30_000_000_000))?;
    // Activation remains pending while the external native preparation is held.
    assert!(matches!(
        session.control.try_close_when_idle(),
        Err(EntryTransportError::InvalidPhase)
    ));
    boundary.issue_release.open();
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ServerActivated { .. })
    })?;
    let sampled = Arc::new(AtomicBool::new(false));
    let observed = sampled.clone();
    rig.clock.on_next_current_thread_observation(Box::new(move || {
        observed.store(true, Ordering::Release);
    }));
    assert!(session.control.try_close_when_idle()?);
    assert!(!sampled.load(Ordering::Acquire));
    assert!(lock(&session.control.shared.state).stopped);
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.authenticated_launches==0)
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    drop(release);
    Ok(())
}

#[test]
fn normal_idle_fence_blocks_reused_endpoint_issue_after_actual_prior_peer_and_loan_retirement()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let release = ReleaseBoundary(boundary.clone());
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { connection, request } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("Original launch missing".into());
    };
    session.control.reply(
        connection,
        request.request_id(),
        EntryOperationId::new(1)?,
        Moment::from_nanos(1_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual==connection),
    )?;
    *lock(&boundary.pause_role) = Some(crate::native_deadlines::Role::Handshake);
    let issued = boundary.connects_issued.load(Ordering::Acquire);
    drop(request);
    boundary.issue_prepared.wait()?;
    assert!(session.control.try_close_when_idle()?);
    boundary.issue_release.open();
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.authenticated_launches==1)
    );
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), issued);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    drop(release);
    Ok(())
}

#[test]
fn normal_idle_fence_rejects_actual_client_role_and_retains_its_terminal_custody() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    let release = ReleaseBoundary(boundary.clone());
    *lock(&boundary.pause_role) = Some(crate::native_deadlines::Role::Handshake);
    let scope = native_scope(&rig)?;
    let mut specification =
        crate::runtime_test_support::spec(1, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let mut session = reserve_client(
        &rig.client.core,
        &scope,
        specification,
        CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?,
        Box::new(Lease(boundary.clone())),
    )?;
    boundary.issue_prepared.wait()?;
    assert!(matches!(
        session.control.try_close_when_idle(),
        Err(EntryTransportError::InvalidPhase)
    ));
    assert!(!lock(&session.control.shared.state).stopped);
    assert!(session.completion.try_take_completion().is_none());
    session.control.close();
    boundary.issue_release.open();
    assert!(matches!(
        rig.completion(&mut session.completion)?.outcome,
        JobOutcome::Success(_) | JobOutcome::Cancelled
    ));
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 0);
    drop(release);
    Ok(())
}
