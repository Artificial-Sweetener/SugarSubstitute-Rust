// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expire real client I/O probes while retaining original staging and server replay custody.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use std::sync::{Arc, atomic::Ordering};
use sugarsubstitute_contracts::{coordinator_entry::*, execution::*};

fn pending_client(
    rig: &Rig,
    id: u64,
) -> TestResult<(Scope, Arc<Boundary>, CoordinatorEntryClientSession)> {
    let boundary = Boundary::new(rig, false)?;
    boundary.defer_client_outcome.store(true, Ordering::Release);
    let mut specification = spec(id, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let input = CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?;
    let scope = native_scope(rig)?;
    let session = crate::coordinator_entry::reserve_client(
        &rig.client.core,
        &scope,
        specification,
        input,
        Box::new(Lease(boundary.clone())),
    )?;
    fact(rig, &session.control, |fact| {
        matches!(
            fact,
            EntryTransportObservation::DiscoveryVerified {
                phase: EntryDiscoveryPhase::Serving(_),
                ..
            }
        )
    })?;
    rig.clock.wait_until_registered(Moment::from_nanos(2_000_000_000))?;
    assert_eq!(boundary.writes.load(Ordering::Acquire), 2);
    Ok((scope, boundary, session))
}

#[test]
fn expired_client_probe_retains_staged_source_while_original_server_loan_and_replay_remain_owned()
-> TestResult {
    let rig = Rig::new()?;
    let server_boundary = Boundary::new(&rig, true)?;
    let (server_scope, mut server_session) = server(&rig, &server_boundary)?;
    activate(&rig, &server_session)?;
    server_boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { connection, request: original } =
        fact(&rig, &server_session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("Actual original server request missing".into());
    };
    let original_id = original.request_id();
    let original_source = original.source_deadline();
    // Both actual jobs share the same native scope; only external endpoint
    // replies are faked, and the original server's charged loan remains live.
    let client_boundary = Boundary::new(&rig, false)?;
    client_boundary.defer_client_outcome.store(true, Ordering::Release);
    let mut specification = spec(2, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let input = CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?;
    let scope = server_session.completion.scope_id();
    let actual_scope = server_scope;
    assert_eq!(actual_scope.id(), scope);
    let mut client_session = crate::coordinator_entry::reserve_client(
        &rig.client.core,
        &actual_scope,
        specification,
        input,
        Box::new(Lease(client_boundary.clone())),
    )?;
    fact(&rig, &client_session.control, |fact| {
        matches!(fact, EntryTransportObservation::DiscoveryVerified { .. })
    })?;
    rig.clock.wait_until_registered(Moment::from_nanos(2_000_000_000))?;
    assert_eq!(client_boundary.writes.load(Ordering::Acquire), 2);
    rig.clock.advance(2_000_000_000);
    fact(&rig, &client_session.control, |fact| {
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    rig.clock.wait_until_registered(Moment::from_nanos(4_000_000_000))?;
    assert!(client_session.completion.try_take_completion().is_none());
    assert_eq!(client_boundary.drops.load(Ordering::Acquire), 0);
    assert_eq!(original.view()?.profile(), "profile_1");
    assert!(
        lock(&server_boundary.memory)
            .as_ref()
            .ok_or("Actual server claim missing")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    assert_eq!(
        client_session.control.shared.original_preparation,
        Moment::from_nanos(30_000_000_000)
    );
    server_session.control.close_connection(connection);
    fact(
        &rig,
        &server_session.control,
        |fact| matches!(fact, EntryTransportObservation::ConnectionRetired { connection: actual } if *actual == connection),
    )?;
    server_session.control.reply(
        connection,
        original_id,
        EntryOperationId::new(1)?,
        Moment::from_nanos(4_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    drop(original);
    server_boundary.connect()?;
    assert!(rejected_connection(&rig, &server_session.control, Some(connection))?.is_empty());
    let replies = lock(&server_boundary.replies);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].0.request, original_id);
    assert_eq!(replies[0].0.source, original_source);
    assert_eq!(replies[0].1, EntryOutcome::Accepted(run()?));
    drop(replies);
    client_boundary.defer_client_outcome.store(false, Ordering::Release);
    rig.clock.advance(2_000_000_000);
    let EntryTransportObservation::OutcomeReceived { request: returned, outcome, .. } =
        fact(&rig, &client_session.control, |fact| {
            matches!(fact, EntryTransportObservation::OutcomeReceived { .. })
        })?
    else {
        return Err("Actual client outcome missing".into());
    };
    assert_eq!(returned, original_id);
    assert_eq!(outcome, EntryOutcome::Accepted(run()?));
    let launches = lock(&client_boundary.client_launches);
    assert_eq!(launches.len(), 2);
    assert_eq!(launches[0].request, launches[1].request);
    assert_eq!(launches[0].source, launches[1].source);
    assert_eq!(launches[1].source, original_source);
    assert_ne!(launches[0].nonce, launches[1].nonce);
    drop(launches);
    assert!(
        matches!(rig.completion(&mut client_session.completion)?.outcome, JobOutcome::Success(report) if report.retired_connections == 2 && report.completed_writes == 4)
    );
    server_session.control.close();
    assert!(
        matches!(rig.completion(&mut server_session.completion)?.outcome, JobOutcome::Success(report) if report.authenticated_launches == 1)
    );
    Ok(())
}

#[test]
fn intentional_client_close_finishes_checked_retirement_without_reprobe() -> TestResult {
    let rig = Rig::new()?;
    let (_scope, boundary, mut session) = pending_client(&rig, 1)?;
    session.control.close();
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(report) if report.completed_writes == 2)
    );
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 1);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn original_client_total_expiry_remains_timed_out_without_reprobe() -> TestResult {
    let rig = Rig::new()?;
    let (_scope, boundary, mut session) = pending_client(&rig, 1)?;
    rig.clock.advance(30_000_000_000);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 1);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn client_quiescence_crossing_probe_discards_late_response_and_reprobes_original_source()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    let clock = rig.clock.clone();
    *lock(&boundary.quiesce_probe) = Some((2, Box::new(move || clock.advance(2_000_000_000))));
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
    // The first complete response was quiesced after its finite deadline. It
    // cannot become an Outcome fact, even though its native bytes were returned.
    fact(&rig, &session.control, |fact| {
        assert!(!matches!(fact, EntryTransportObservation::OutcomeReceived { .. }));
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    rig.clock.wait_until_registered(Moment::from_nanos(4_000_000_000))?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert_eq!(boundary.writes.load(Ordering::Acquire), 2);
    rig.clock.advance(2_000_000_000);
    let EntryTransportObservation::OutcomeReceived { request, outcome, .. } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::OutcomeReceived { .. })
        })?
    else {
        return Err("Timely reprobe outcome missing".into());
    };
    assert_eq!(request.get(), source()?.ticks() as u64);
    assert_eq!(outcome, EntryOutcome::Accepted(run()?));
    let launches = lock(&boundary.client_launches);
    assert_eq!(launches.len(), 2);
    assert_eq!(launches[0].request, launches[1].request);
    assert_eq!(launches[0].source, launches[1].source);
    assert_ne!(launches[0].nonce, launches[1].nonce);
    drop(launches);
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(report) if report.retired_connections == 2 && report.completed_writes == 4)
    );
    Ok(())
}
