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
    client_with_recovery(rig, boundary, false)
}

fn client_with_recovery(
    rig: &Rig,
    boundary: &Arc<Boundary>,
    recovery: bool,
) -> TestResult<(Scope, CoordinatorEntryClientSession)> {
    let scope = native_scope(rig)?;
    let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let mut input = CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?;
    if recovery {
        input = input.with_unsubmitted_recovery();
    }
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
fn never_connected_source_returns_only_after_checked_retirement_and_original_probe() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    boundary.absent_endpoint.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let release = ReleaseBoundary(boundary.clone());
    let (scope, mut session) = client_with_recovery(&rig, &boundary, true)?;
    boundary.retiring.wait()?;
    assert!(session.control.take_unsubmitted_request().is_none());
    assert!(session.completion.try_take_completion().is_none());
    drop(release);
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    rig.clock.wait_until_registered(Moment::from_nanos(2_000_000_000))?;
    assert!(session.control.take_unsubmitted_request().is_none());
    rig.clock.advance(2_000_000_000);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    let recovered = session.control.take_unsubmitted_request().ok_or("Source custody lost")?;
    recovered.validate(
        &facts()?,
        None,
        source()?,
        &rig.client,
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?,
    )?;
    assert!(scope.snapshot().reserved_bytes > 0);
    let recovered = recovered.into_request();
    assert_eq!(recovered.units(), request()?.environment());
    assert_eq!(boundary.connects_issued.load(Ordering::Acquire), 1);
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(session.control.take_unsubmitted_request().is_none());
    drop(recovered);
    Ok(())
}

#[test]
fn cancellation_of_absent_endpoint_never_returns_source_for_fresh_admission() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    boundary.absent_endpoint.store(true, Ordering::Release);
    let (_scope, mut session) = client_with_recovery(&rig, &boundary, true)?;
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    session.completion.cancel();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert!(session.control.take_unsubmitted_request().is_none());
    Ok(())
}

#[test]
fn issued_launch_permanently_consumes_source_handback() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    boundary.closing_after_launch.store(true, Ordering::Release);
    let (_scope, mut session) = client_with_recovery(&rig, &boundary, true)?;
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    assert!(session.control.take_unsubmitted_request().is_none());
    assert_eq!(lock(&boundary.client_launches).len(), 1);
    Ok(())
}

#[test]
fn authenticated_closing_before_launch_returns_original_unsubmitted_source() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, false)?;
    boundary.closing_before_launch.store(true, Ordering::Release);
    let (_scope, mut session) = client_with_recovery(&rig, &boundary, true)?;
    fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::ConnectionRetired { .. })
    })?;
    rig.clock.wait_until_registered(Moment::from_nanos(2_000_000_000))?;
    assert!(session.control.take_unsubmitted_request().is_none());
    rig.clock.advance(2_000_000_000);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    let recovered = session.control.take_unsubmitted_request().ok_or("Pre-Launch source lost")?;
    recovered.validate(
        &facts()?,
        None,
        source()?,
        &rig.client,
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?,
    )?;
    assert_eq!(recovered.input.source_setup, source()?);
    assert_eq!(recovered.into_request().units(), request()?.environment());
    assert_eq!(boundary.writes.load(Ordering::Acquire), 1);
    assert!(lock(&boundary.client_launches).is_empty());
    Ok(())
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
