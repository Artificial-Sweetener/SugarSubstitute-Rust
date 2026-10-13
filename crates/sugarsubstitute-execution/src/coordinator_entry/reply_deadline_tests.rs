// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep original setup custody distinct from one bounded authenticated reply write.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult},
    scope::lock,
};
use std::{sync::atomic::Ordering, time::Duration};
use sugarsubstitute_contracts::{coordinator_entry::*, execution::*};

#[test]
fn original_setup_cutoff_rejects_reply_without_consumption_then_current_probe_preserves_source_and_run()
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
        return Err("Actual authenticated original request missing".into());
    };
    let original_until = request.until();
    let original_source = request.source_deadline();
    let original_request = request.request_id();
    let operation = EntryOperationId::new(1)?;
    let selected = run()?;
    let before = {
        let state = lock(&session.control.shared.state);
        let index = state.index(connection)?;
        let slot = &state.connections[index];
        (slot.latest, slot.reserved, slot.observed)
    };
    let rejected = session.control.reply(
        connection,
        original_request,
        operation,
        original_until,
        EntryOutcome::Accepted(selected),
    );
    assert!(matches!(rejected, Err(EntryTransportError::InvalidDeadline)));
    assert_eq!(boundary.writes.load(Ordering::Acquire), 1);
    assert!(lock(&boundary.replies).is_empty());
    {
        let state = lock(&session.control.shared.state);
        let index = state.index(connection)?;
        let slot = &state.connections[index];
        assert_eq!((slot.latest, slot.reserved, slot.observed), before);
        let route = state
            .requests
            .iter()
            .flatten()
            .find(|route| route.connection == connection && route.request == original_request)
            .ok_or("Actual pending request was consumed")?;
        assert_eq!(route.latest, 0);
        assert!(route.reply.is_none());
        assert!(slot.loan);
    }
    let now = rig.client.now()?;
    let probe = now.checked_add(Duration::from_secs(2))?.min(original_until);
    assert!(probe < original_until);
    session.control.reply(
        connection,
        original_request,
        operation,
        probe,
        EntryOutcome::Accepted(selected),
    )?;
    let retired = fact(
        &rig,
        &session.control,
        |fact| matches!(fact, EntryTransportObservation::ConnectionRetired { connection: actual, .. } if *actual == connection),
    )?;
    assert!(matches!(retired, EntryTransportObservation::ConnectionRetired { .. }));
    let replies = lock(&boundary.replies);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].0.request, original_request);
    assert_eq!(replies[0].0.source, original_source);
    assert_eq!(replies[0].1, EntryOutcome::Accepted(selected));
    drop(replies);
    assert_eq!(request.until(), original_until);
    assert_eq!(request.source_deadline(), original_source);
    assert_eq!(request.view()?.profile(), "profile_1");
    assert!(session.completion.try_take_completion().is_none());
    drop(request);
    session.control.close();
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(report) if report.authenticated_launches == 1 && report.completed_writes == 2)
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}
