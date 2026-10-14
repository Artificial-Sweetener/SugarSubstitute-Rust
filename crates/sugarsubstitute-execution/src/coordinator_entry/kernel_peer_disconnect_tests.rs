// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the settled Root cache across actual connection verification failures.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult},
    scope::lock,
};
use std::{error::Error, num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::{coordinator_entry::*, execution::*};
fn query_error(code: u32) -> EntryTransportError {
    let cause = NativeObservationError::new(
        "entry_query_kernel_peer",
        windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(code)),
    );
    if code == 233 {
        EntryTransportError::PeerDisconnected(cause)
    } else {
        EntryTransportError::native(cause)
    }
}
#[test]
fn kernel_peer_loss_after_verified_connect_preserves_cached_run_and_serves_next_clients()
-> TestResult {
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
        return Err("Original request missing".into());
    };
    let id = request.request_id();
    let source = request.source_deadline();
    let until = request.until();
    session.control.reply(
        connection,
        id,
        EntryOperationId::new(1)?,
        Moment::from_nanos(2_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual==connection),
    )?;
    *lock(&boundary.verify_failure) = Some(query_error(233));
    boundary.connect_source_at(1, source)?;
    let verified = std::cell::Cell::new(None);
    let retired = fact(&rig, &session.control, |fact| {
        if let EntryTransportObservation::ConnectionVerified { connection, .. } = fact {
            verified.set(Some(*connection));
        }
        assert!(!matches!(fact, EntryTransportObservation::RequestAuthenticated { .. }));
        matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual!=connection)
    })?;
    let EntryTransportObservation::ConnectionRetired { connection: lost, failure: Some(cause) } =
        retired
    else {
        return Err("Original kernel-query cause missing".into());
    };
    assert_eq!(verified.get(), Some(lost));
    assert!(matches!(cause, EntryTransportError::PeerDisconnected(_)));
    assert_eq!(
        cause
            .native_cause()
            .and_then(Error::source)
            .and_then(|e| e.downcast_ref::<windows::core::Error>())
            .map(windows::core::Error::code),
        Some(windows::core::HRESULT::from_win32(233))
    );
    assert!(rig.client.drain_completions(NonZeroUsize::MIN).is_ok());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(request.until(), until);
    boundary.connect_source_at(2, source)?;
    fact(&rig, &session.control, |fact| {
        assert!(!matches!(fact, EntryTransportObservation::RequestAuthenticated { .. }));
        matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual!=connection && *actual!=lost)
    })?;
    {
        let replies = lock(&boundary.replies);
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[1].0.request, id);
        assert_eq!(replies[1].0.source, source);
        assert_eq!(replies[1].1, EntryOutcome::Accepted(run()?));
    }
    boundary.connect_at(3)?;
    let EntryTransportObservation::RequestAuthenticated {
        connection: other,
        request: other_request,
    } = fact(&rig, &session.control, |fact| {
        matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
    })?
    else {
        return Err("Distinct client missing".into());
    };
    assert_ne!(other_request.request_id(), id);
    session.control.reply(
        other,
        other_request.request_id(),
        EntryOperationId::new(2)?,
        Moment::from_nanos(2_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual==other),
    )?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    drop(request);
    drop(other_request);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report)
        if report.authenticated_launches==2 && report.retired_connections==4 && report.completed_writes==6));
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn unexpected_kernel_peer_query_preserves_fatal_mailbox_cause_and_requires_composing_cleanup()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    *lock(&boundary.verify_failure) = Some(query_error(5));
    boundary.connect()?;
    let failure = loop {
        let observed = rig.wake.count();
        if let Err(cause) = rig.client.drain_completions(NonZeroUsize::MIN) {
            break cause;
        }
        rig.wake.wait_after(observed)?;
    };
    let mut actual: Option<&(dyn Error + 'static)> = Some(&failure);
    let mut found = false;
    for _ in 0..16 {
        let Some(cause) = actual else {
            break;
        };
        if let Some(native) = cause.downcast_ref::<windows::core::Error>() {
            assert_eq!(native.code(), windows::core::HRESULT::from_win32(5));
            found = true;
        }
        actual = cause.source();
    }
    assert!(found);
    assert!(session.completion.try_take_completion().is_none());
    session.control.close();
    boundary.retiring.wait()?;
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn final_reply_keeps_endpoint_and_original_launch_loan_until_actual_peer_close() -> TestResult {
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
        |fact| matches!(fact,EntryTransportObservation::Written {operation,..} if operation.get()==1),
    )?;
    boundary.peer_retirement_issued.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(request.until(), original);
    assert_eq!(request.view()?.profile(), "profile_1");
    assert_eq!(boundary.native_closes.load(Ordering::Acquire), 0);
    boundary.close_peer(0)?;
    let retired = fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual==connection),
    )?;
    assert!(matches!(
        retired,
        EntryTransportObservation::ConnectionRetired {
            failure: Some(EntryTransportError::PeerDisconnected(_)),
            ..
        }
    ));
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    drop(request);
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.authenticated_launches==1 && report.retired_connections==1)
    );
    Ok(())
}
#[test]
fn final_reply_peer_wait_expires_at_original_write_cutoff_without_renewal_or_new_admission()
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
    session.control.reply(
        connection,
        request.request_id(),
        EntryOperationId::new(1)?,
        Moment::from_nanos(1_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    fact(&rig, &session.control, |fact| matches!(fact, EntryTransportObservation::Written { .. }))?;
    boundary.peer_retirement_issued.wait()?;
    rig.clock.advance(1_000_000_000);
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact,EntryTransportObservation::ConnectionRetired {connection:actual,..} if *actual==connection),
    )?;
    assert_eq!(lock(&boundary.replies).len(), 1);
    assert!(session.completion.try_take_completion().is_none());
    session.control.close();
    boundary.retiring.wait()?;
    drop(request);
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.authenticated_launches==1 && report.retired_connections==1)
    );
    Ok(())
}
