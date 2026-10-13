// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reprobe actual endpoint absence without authenticating a peer or renewing source custody.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use std::{num::NonZeroUsize, sync::Arc, sync::atomic::Ordering};
use sugarsubstitute_contracts::execution::*;

fn client(
    rig: &Rig,
    boundary: &Arc<Boundary>,
) -> TestResult<(Scope, CoordinatorEntryClientSession)> {
    let scope = native_scope(rig)?;
    let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let input = CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?;
    let session = reserve_client(
        &rig.client.core,
        &scope,
        specification,
        input,
        Box::new(Lease(boundary.clone())),
    )?;
    Ok((scope, session))
}

#[test]
fn absent_endpoint_waits_for_checked_retirement_then_original_probe_and_source() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    boundary.absent_endpoint.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let release = ReleaseBoundary(boundary.clone());
    let (scope, mut session) = client(&rig, &boundary)?;
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 1);
    assert_eq!(boundary.native_closes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, MIN_ENTRY_CLIENT_BYTES);
    drop(release);
    let EntryTransportObservation::ConnectionRetired { connection, failure: Some(cause) } =
        fact(&rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
        })?
    else {
        return Err("Missing exact retired absence cause".into());
    };
    assert!(matches!(cause, EntryTransportError::EndpointAbsent(_)));
    assert_eq!(connection.get(), 1);
    assert_eq!(cause.native_cause().map(|cause| cause.stage()), Some("entry_open_client"));
    rig.clock.wait_until_registered(Moment::from_nanos(2_000_000_000))?;
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 1);
    boundary.absent_endpoint.store(false, Ordering::Release);
    rig.clock.advance(2_000_000_000);
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::OutcomeReceived { .. })
    })?;
    let headers = lock(&boundary.client_launches);
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].source, source()?);
    assert_eq!(headers[0].request.get(), source()?.ticks() as u64);
    drop(headers);
    let completed = rig.completion(&mut session.completion)?;
    assert!(
        matches!(completed.outcome, JobOutcome::Success(report) if report.retired_connections == 2)
    );
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 2);
    Ok(())
}

#[test]
fn persistently_absent_endpoint_stops_at_original_total_cutoff_without_authority() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    boundary.absent_endpoint.store(true, Ordering::Release);
    let (_scope, mut session) = client(&rig, &boundary)?;
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    rig.clock.wait_until_registered(Moment::from_nanos(2_000_000_000))?;
    rig.clock.advance(30_000_000_000);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 1);
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(lock(&boundary.client_launches).is_empty());
    Ok(())
}

#[test]
fn unexpected_client_open_still_delivers_fatal_original_error_without_reprobe() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    *lock(&boundary.connect_failure) = Some("entry_open_client");
    let (_scope, mut session) = client(&rig, &boundary)?;
    let mut observed_failure = false;
    let completed = loop {
        let observed = rig.wake.count();
        if rig.client.drain_completions(NonZeroUsize::MIN).is_err() {
            observed_failure = true;
            // Failure delivery leaves the same coalesced activity batch pending.
            // Drain it before waiting for an additional external wake.
            continue;
        }
        if let Some(completed) = session.completion.try_take_completion() {
            break completed;
        }
        rig.wake.wait_after(observed)?;
    };
    let JobOutcome::Failure(cause) = completed.outcome else {
        return Err("Fatal native error lost".into());
    };
    assert_eq!(cause.native_cause().map(|cause| cause.stage()), Some("entry_open_client"));
    assert!(observed_failure);
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 1);
    assert!(lock(&boundary.client_launches).is_empty());
    Ok(())
}
