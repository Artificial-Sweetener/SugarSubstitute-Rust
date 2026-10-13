// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire a closed requester without discarding its settled replay or charged frame.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult},
    scope::lock,
};
use std::{error::Error, num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::{coordinator_entry::*, execution::*};

#[test]
fn closed_requester_reply_retires_locally_and_fresh_nonce_replays_same_run_without_readmission()
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
        return Err("Actual original request missing".into());
    };
    let original_id = request.request_id();
    let original_source = request.source_deadline();
    let original_until = request.until();
    // The requester has closed its native endpoint; Root finishes real admission
    // after the client probe, while the original source cutoff remains live.
    rig.clock.advance(3_000_000_000);
    *lock(&boundary.outcome_write_failure) =
        Some(EntryTransportError::PeerDisconnected(NativeObservationError::new(
            "pipe_issue",
            std::io::Error::other("closed original requester"),
        )));
    session.control.reply(
        connection,
        original_id,
        EntryOperationId::new(1)?,
        Moment::from_nanos(5_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    let EntryTransportObservation::ConnectionRetired { connection: retired, failure } = fact(
        &rig,
        &session.control,
        |fact| matches!(fact, EntryTransportObservation::ConnectionRetired { connection: actual, .. } if *actual == connection),
    )?
    else {
        return Err("Checked original retirement missing".into());
    };
    assert_eq!(retired, connection);
    let failure = failure.ok_or("Original write cause lost")?;
    assert!(matches!(failure, EntryTransportError::PeerDisconnected(_)));
    assert_eq!(failure.native_cause().map(NativeObservationError::stage), Some("pipe_issue"));
    assert_eq!(
        failure.source().and_then(Error::source).map(ToString::to_string),
        Some("closed original requester".into())
    );
    assert!(lock(&session.control.shared.state).requests.iter().all(Option::is_none));
    assert!(rig.client.drain_completions(NonZeroUsize::MIN).is_ok());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(request.view()?.profile(), "profile_1");
    assert_eq!(request.until(), original_until);
    assert!(
        lock(&boundary.memory)
            .as_ref()
            .ok_or("Original lease missing")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    boundary.connect_source_at(1, original_source)?;
    let EntryTransportObservation::ConnectionRetired { failure: fresh_failure, .. } = fact(
        &rig,
        &session.control,
        |fact| {
            assert!(!matches!(fact, EntryTransportObservation::RequestAuthenticated { .. }));
            matches!(fact, EntryTransportObservation::ConnectionRetired { connection: actual, .. } if *actual != connection)
        },
    )?
    else {
        return Err("Fresh cached reply retirement missing".into());
    };
    assert!(fresh_failure.is_none());
    let replies = lock(&boundary.replies);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].0.request, original_id);
    assert_eq!(replies[0].0.source, original_source);
    assert_eq!(replies[0].0.nonce, [2; 32]);
    assert_eq!(replies[0].1, EntryOutcome::Accepted(run()?));
    drop(replies);
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert_eq!(request.view()?.profile(), "profile_1");
    drop(request);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(report)
        if report.authenticated_launches == 1 && report.retired_connections == 2 && report.completed_writes == 3));
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn unexpected_reply_write_still_fails_whole_owner_with_original_cause() -> TestResult {
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
        return Err("Actual request missing".into());
    };
    *lock(&boundary.outcome_write_failure) =
        Some(EntryTransportError::native(NativeObservationError::new(
            "unexpected_reply_write",
            std::io::Error::other("actual external failure"),
        )));
    session.control.reply(
        connection,
        request.request_id(),
        EntryOperationId::new(1)?,
        Moment::from_nanos(2_000_000_000),
        EntryOutcome::Accepted(run()?),
    )?;
    boundary.retiring.wait()?;
    drop(request);
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
    assert!(matches!(completion.outcome, JobOutcome::Failure(ref cause)
        if cause.native_cause().is_some_and(|cause| cause.stage() == "unexpected_reply_write")));
    assert!(!failures.is_empty() || rig.client.drain_completions(NonZeroUsize::MIN).is_err());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}
