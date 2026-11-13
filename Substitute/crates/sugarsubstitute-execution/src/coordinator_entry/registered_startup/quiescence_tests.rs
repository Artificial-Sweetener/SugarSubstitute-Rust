// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Refuse original archived restart authority after the actual normal-idle fence.
use super::*;
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::coordinator_entry::{EntryOperationId, EntryOutcome};

#[test]
fn actual_idle_fence_vetoes_archived_restart_without_consuming_original_source() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    boundary.hold_peer_close.store(true, Ordering::Release);
    let (_scope, mut session) = server(&rig, &boundary)?;
    let startup = request_snapshot(&rig, &boundary, &session)?;
    let launch = startup.launch.as_ref().ok_or("Actual authenticated launch missing")?;
    let connection = launch.peer().connection();
    let request = launch.request_id();
    let until = launch.until();
    let reply_until = crate::coordinator_entry::schedule::probe(rig.client.now()?, until)?;
    let (source, memory) = registered(&rig, startup)?;
    drop(memory);
    assert!(!session.control.try_close_when_idle()?);
    session.control.reply(
        connection,
        request,
        EntryOperationId::new(1)?,
        reply_until,
        EntryOutcome::Accepted(run()?),
    )?;
    fact(&rig, &session.control, |fact| matches!(fact, EntryTransportObservation::Written { .. }))?;
    boundary.peer_retirement_issued.wait()?;
    assert!(!session.control.try_close_when_idle()?);
    boundary.close_peer(0)?;
    fact(
        &rig,
        &session.control,
        |fact| matches!(fact, EntryTransportObservation::ConnectionRetired { connection: actual, .. } if *actual == connection),
    )?;
    assert!(session.control.try_close_when_idle()?);
    let previous = run()?;
    let new = next(previous)?;
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let client = rig.client.clone();
    let retained = source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        projection_spec(3, original.until())?,
        move |context| {
            let rejected =
                client.restart_startup_snapshot(&retained, previous, new, original, &context);
            assert!(rejected.is_err());
            assert_eq!(lock(&retained.state()?.progress).current, previous);
            assert!(retained.same_source(&source));
            assert_eq!(retained.retained_bytes(), REGISTERED_STARTUP_BYTES);
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(retained)
        },
    )?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(retained) = completion.outcome else {
        return Err("Original restart rejection worker failed".into());
    };
    assert_eq!(retained.view()?.profile(), "profile_1");
    drop(completion.memory);
    drop(retained);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
