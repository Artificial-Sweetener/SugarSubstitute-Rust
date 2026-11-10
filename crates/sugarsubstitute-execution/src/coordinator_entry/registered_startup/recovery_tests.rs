// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise recovery grants through real Entry authentication, archives and central workers.
use super::*;
use sugarsubstitute_contracts::{incident::*, recovery_supervisor::*};
struct Projected {
    snapshot: Option<PreparedStartupSnapshot>,
}
type Completed = JobCompletion<Projected, AdmissionError>;
fn issued(rig: &Rig, source: &RegisteredStartupSource) -> TestResult<Completed> {
    let previous = run()?;
    let run = next(previous)?;
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let retained = source.clone();
    let client = rig.client.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        projection_spec(3, original.until())?,
        move |context| {
            client
                .restart_startup_snapshot(&retained, previous, run, original, &context)
                .map(|snapshot| Projected { snapshot: Some(snapshot) })
        },
    )?;
    rig.completion(&mut job)
}
fn take(completed: &mut Completed) -> TestResult<PreparedStartupSnapshot> {
    match &mut completed.outcome {
        JobOutcome::Success(value) => {
            value.snapshot.take().ok_or("Original snapshot missing".into())
        }
        _ => Err("Actual restart projection failed".into()),
    }
}
fn key() -> TestResult<RecoveryGrantKey> {
    Ok(RecoveryGrantKey::new(
        RecoveryGrantId::new(1)?,
        run()?,
        ReportBinding::new(
            IncidentKey::new(IncidentSubject::Desktop(run()?)),
            IncidentRevision::new(1)?,
            ReportSessionId::new(1)?,
        ),
    )?)
}
fn close(rig: &Rig, session: &mut CoordinatorEntryServerSession) -> TestResult {
    session.control.close();
    drop(rig.completion(&mut session.completion)?);
    Ok(())
}
#[test]
fn recovery_grant_consumes_actual_snapshot_preserves_exact_original_credit_and_blocks_direct_capture()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, original_memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(original_memory);
    let mut completed = issued(&rig, &source)?;
    let snapshot = take(&mut completed)?;
    let direct_binding = snapshot.admission_binding()?;
    let expected = snapshot.body()?.to_vec();
    let before = rig.scope.snapshot().reserved_bytes;
    let grant =
        rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
    grant.check()?;
    assert_eq!(grant.key(), key()?);
    assert_eq!(grant.state()?.startup_body(), expected);
    assert_eq!(
        grant.original_budget().until(),
        completed.spec.deadline.ok_or("Actual cutoff missing")?
    );
    assert!(completed.memory.claim(BYTES - 4096).is_ok());
    assert!(completed.memory.claim(BYTES - 4096 + 1).is_err());
    assert!(
        direct_binding
            .validate_current_locked(
                &rig.client.core,
                &lock(&rig.client.core.state),
                key()?.run(),
                rig.client.now()?
            )
            .is_err()
    );
    let alias = grant.clone();
    drop(direct_binding);
    drop(completed);
    drop(grant);
    assert_eq!(rig.scope.snapshot().reserved_bytes, before);
    alias.check()?;
    drop(alias);
    // Typed terminal publication precedes the supervisor's final context retirement.
    close(&rig, &mut session)?;
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, before - BYTES);
    drop(source);
    Ok(())
}
#[test]
fn recovery_grant_rejects_foreign_key_memory_and_core_without_latching_or_consuming_source()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    let mut completed = issued(&rig, &source)?;
    let mut snapshot = take(&mut completed)?;
    let foreign = Rig::new()?;
    let failure = foreign
        .client
        .issue_registered_recovery_grant(snapshot, &source, &completed, key()?)
        .err()
        .ok_or("Foreign Core issued grant")?;
    snapshot = failure.snapshot;
    let copied = JobCompletion {
        scope_id: completed.scope_id,
        spec: completed.spec,
        queue_time: completed.queue_time,
        execution_time: completed.execution_time,
        outcome: JobOutcome::<(), AdmissionError>::Success(()),
        memory: memory.clone(),
        native_handles: None,
    };
    let failure = rig
        .client
        .issue_registered_recovery_grant(snapshot, &source, &copied, key()?)
        .err()
        .ok_or("Foreign memory issued grant")?;
    snapshot = failure.snapshot;
    drop(copied);
    drop(memory);
    let mut previous = key()?.previous();
    previous.instance = InstanceId::new(99)?;
    let changed = RecoveryGrantKey::new(RecoveryGrantId::new(1)?, previous, key()?.report())?;
    let failure = rig
        .client
        .issue_registered_recovery_grant(snapshot, &source, &completed, changed)
        .err()
        .ok_or("Changed run issued grant")?;
    snapshot = failure.snapshot;
    assert!(
        source.state()?.progress.lock().map_err(|_| "Progress poisoned")?.recovery_issued.is_none()
    );
    assert!(completed.memory.claim(BYTES).is_ok());
    let grant =
        rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
    grant.check()?;
    drop(grant);
    drop(completed);
    drop(source);
    close(&rig, &mut session)
}
#[test]
fn recovery_grant_once_only_latch_survives_alias_drop_and_never_reissues_same_generation()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let mut completed = issued(&rig, &source)?;
    let snapshot = take(&mut completed)?;
    let original = snapshot.registered.as_ref().ok_or("Registered source missing")?.clone();
    let grant =
        rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
    drop(grant);
    let copied = PreparedStartupSnapshot::registered(original)?;
    let failure = rig
        .client
        .issue_registered_recovery_grant(copied, &source, &completed, key()?)
        .err()
        .ok_or("Same generation reissued")?;
    drop(failure);
    drop(completed);
    drop(source);
    close(&rig, &mut session)
}
#[test]
fn recovery_grant_original_cutoff_and_actual_root_entry_cancellation_veto_current_effects()
-> TestResult {
    for cancel in [false, true] {
        let rig = Rig::new()?;
        let boundary = Boundary::new(&rig, true)?;
        let (entry_scope, mut session) = server(&rig, &boundary)?;
        let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
        drop(memory);
        let mut completed = issued(&rig, &source)?;
        let snapshot = take(&mut completed)?;
        let grant =
            rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
        grant.check()?;
        if cancel {
            rig.client.core.cancel_scope(entry_scope.id());
        } else {
            rig.clock.advance(grant.original_budget().until().as_nanos());
        }
        assert!(grant.check().is_err());
        assert!(
            grant.state()?.stages.lock().map_err(|_| "Grant stages poisoned")?.creation.is_none()
        );
        drop(grant);
        drop(completed);
        drop(source);
        close(&rig, &mut session)?;
    }
    Ok(())
}

#[path = "recovery_creation_tests.rs"]
mod creation;

#[path = "recovery_startup_tests.rs"]
mod startup;

#[test]
fn recovery_grant_final_control_retires_before_original_claimed_payload_and_all_aliases_stay_closed()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, original_memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(original_memory);
    let mut completed = issued(&rig, &source)?;
    let snapshot = take(&mut completed)?;
    let grant =
        rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
    let before = rig.scope.snapshot().reserved_bytes;
    crate::coordinator_entry::recovery_grant::control_tests::assert_final_payload_custody(
        grant,
        &completed.memory,
        &rig.scope,
    )?;
    drop(completed);
    // Typed terminal publication precedes the supervisor's final context retirement.
    close(&rig, &mut session)?;
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, before - BYTES);
    drop(source);
    Ok(())
}

#[test]
fn recovery_grant_keeps_original_view_and_maps_the_selected_child_profile() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let snapshot = request_snapshot(&rig, &boundary, &session)?;
    let original = snapshot.original_body()?.to_vec();
    let profile = request()?.profile_directory().join("user").join("desktop-data").join("profile");
    let (source, memory) = registered_profile(&rig, snapshot, profile)?;
    drop(memory);
    let expected = source.state()?.child_body.to_vec();
    let mut completed = issued(&rig, &source)?;
    let snapshot = take(&mut completed)?;
    let grant =
        rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
    assert_ne!(original, expected);
    assert_eq!(grant.state()?.startup_body(), expected);
    assert_eq!(grant.startup_bytes(), expected.len());
    assert_eq!(
        grant.view()?.profile_directory().le_bytes(),
        source.view()?.profile_directory().le_bytes()
    );
    assert_eq!(source.state()?.body.as_ref(), original);
    grant.check()?;
    drop(grant);
    drop(completed);
    drop(source);
    close(&rig, &mut session)?;
    Ok(())
}
