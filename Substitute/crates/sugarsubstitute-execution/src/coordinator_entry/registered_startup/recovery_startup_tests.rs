// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Receive actual guarded native messages before publishing genuine delegated creation custody.
use super::*;
use crate::{
    lifecycle_pipe::tests::{recovery_child, recovery_frame},
    process_creation::*,
    recovery_supervisor::*,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_contracts::recovery_supervisor::wire::*;
fn assembly(
    rig: &Rig,
    control: &RecoveryPipeControl,
    id: u64,
) -> TestResult<RecoveryStartupAssembly> {
    let mut spec = projection_spec(id, control.original_deadline())?;
    spec.reservation_bytes = BYTES;
    let mut job = rig.client.submit_recovery_startup_assembly(&rig.scope, spec, control)?;
    match rig.completion(&mut job)?.outcome {
        JobOutcome::Success(value) => Ok(value),
        _ => Err("Actual startup assembly producer failed".into()),
    }
}
/// Build bytes through the unchanged Launch owner; authenticated pipe guards supply provenance.
fn launch_body(rig: &Rig) -> TestResult<(Scope, CoordinatorEntryServerSession, Vec<u8>)> {
    let boundary = Boundary::new(rig, true)?;
    let (scope, session) = server(rig, &boundary)?;
    let startup = request_snapshot(rig, &boundary, &session)?;
    let body = startup.body()?.to_vec();
    drop(startup);
    Ok((scope, session, body))
}
fn desktop_identity() -> TestResult<RecoveryCommand<'static>> {
    Ok(RecoveryCommand::DesktopIdentity {
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
fn commit(key: RecoveryGrantKey) -> TestResult<RecoveryCommand<'static>> {
    Ok(RecoveryCommand::Commit(DurableAcknowledgement::new(
        key.report().incident(),
        key.report().revision().next()?,
        IncidentUtc::from_unix_seconds(1)?,
    )))
}
#[test]
fn recovery_startup_requires_complete_same_session_body_root_and_exact_once_ack_before_origin()
-> TestResult {
    let rig = Rig::new()?;
    let (_entry_scope, mut entry, body) = launch_body(&rig)?;
    let (_scope, mut session, boundary) = recovery_child(&rig, 3, 4, key()?)?;
    let mut owner = assembly(&rig, session.control(), 5)?;
    // The competing producer is rejected by the actual once-only source before it publishes an origin.
    let mut competing = rig.client.submit_recovery_startup_assembly(
        &rig.scope,
        projection_spec(7, session.control().original_deadline())?,
        session.control(),
    )?;
    assert!(matches!(rig.completion(&mut competing)?.outcome, JobOutcome::Failure(_)));
    let early = recovery_frame(&rig, &mut session, &boundary, 1, key()?, 1, commit(key()?)?)?;
    assert!(matches!(owner.receive(&early), Err(RecoveryStartupError::Order)));
    drop(early);
    let identity =
        recovery_frame(&rig, &mut session, &boundary, 2, key()?, 1, desktop_identity()?)?;
    owner.receive(&identity)?;
    assert!(matches!(owner.receive(&identity), Err(RecoveryStartupError::Order)));
    drop(identity);
    let received = recovery_frame(
        &rig,
        &mut session,
        &boundary,
        3,
        key()?,
        2,
        RecoveryCommand::Startup { total: body.len() as u32, offset: 0, bytes: &body },
    )?;
    owner.receive(&received)?;
    assert!(matches!(owner.receive(&received), Err(RecoveryStartupError::Order)));
    drop(received);
    let rejected = owner.finish().err().ok_or("Missing Root path published origin")?;
    owner = rejected.1;
    let (_foreign_scope, mut foreign, foreign_boundary) = recovery_child(&rig, 4, 8, key()?)?;
    let foreign_frame = recovery_frame(
        &rig,
        &mut foreign,
        &foreign_boundary,
        1,
        key()?,
        3,
        RecoveryCommand::CaptureRoot { identity: [4; 24], total: 2, offset: 0, units_le: &[65, 0] },
    )?;
    assert!(matches!(owner.receive(&foreign_frame), Err(RecoveryStartupError::Binding)));
    drop(foreign_frame);
    let root = recovery_frame(
        &rig,
        &mut session,
        &boundary,
        4,
        key()?,
        3,
        RecoveryCommand::CaptureRoot { identity: [4; 24], total: 2, offset: 0, units_le: &[65, 0] },
    )?;
    owner.receive(&root)?;
    drop(root);
    let committed = recovery_frame(&rig, &mut session, &boundary, 5, key()?, 4, commit(key()?)?)?;
    owner.receive(&committed)?;
    drop(committed);
    let (prepared, sequence) = owner.finish().map_err(|(e, _)| e)?;
    assert_eq!(sequence, 4);
    assert_eq!(prepared.origin.key(), key()?);
    assert_eq!(prepared.startup.body()?, body);
    assert_eq!(prepared.origin.capture_root_units().collect::<Vec<_>>(), vec![65]);
    assert_eq!(prepared.origin.capture_root_identity(), [4; 24]);
    prepared.origin.check()?;
    session.control.close();
    assert!(prepared.origin.check().is_err());
    drop(prepared);
    drop(rig.completion(&mut session.completion)?);
    drop(session);
    drop(boundary);
    foreign.control.close();
    drop(rig.completion(&mut foreign.completion)?);
    drop(foreign);
    drop(foreign_boundary);
    close(&rig, &mut entry)
}
#[test]
fn recovery_startup_cancellation_original_expiry_and_foreign_core_cannot_publish_delegation()
-> TestResult {
    for expired in [false, true] {
        let rig = Rig::new()?;
        let (_scope, mut session, boundary) = recovery_child(&rig, 3, 4, key()?)?;
        let owner = assembly(&rig, session.control(), 5)?;
        let other = Rig::new()?;
        assert!(
            other
                .client
                .submit_recovery_startup_assembly(
                    &other.scope,
                    projection_spec(6, session.control().original_deadline())?,
                    session.control()
                )
                .is_err()
        );
        if expired {
            rig.clock.advance(30_000_000_000);
        } else {
            session.control.close();
        }
        let (cause, owner) = owner.finish().err().ok_or("Stopped source published origin")?;
        assert!(matches!(cause, RecoveryStartupError::Admission(_)));
        drop(owner);
        session.control.close();
        drop(rig.completion(&mut session.completion)?);
        drop(session);
        drop(boundary);
    }
    Ok(())
}

struct CreationDriver(Arc<AtomicUsize>);
impl ProcessCreationDriver for CreationDriver {
    fn initialize(
        &mut self,
        _: &JobContext,
        memory: &ProcessCreationMemory,
        _: ProcessCreationWake,
    ) -> Result<(), crate::native::NativeObservationError> {
        memory.check_effect().map_err(|e| {
            crate::native::NativeObservationError::new("delegated_creation_test", e)
        })?;
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
    fn progress(
        &mut self,
        _: &JobContext,
        _: Moment,
    ) -> Result<ProcessCreationProgress, crate::native::NativeObservationError> {
        Ok(ProcessCreationProgress::Started)
    }
    fn retire(
        &mut self,
        _: &JobContext,
        _: Moment,
    ) -> Result<bool, crate::native::NativeObservationError> {
        Ok(true)
    }
}
#[test]
fn delegated_creation_uses_actual_successful_source_once_and_rejects_foreign_or_renewed_admission_without_effects()
-> TestResult {
    let rig = Rig::new()?;
    let (_entry_scope, mut entry, body) = launch_body(&rig)?;
    let (_scope, mut session, boundary) = recovery_child(&rig, 3, 4, key()?)?;
    let mut owner = assembly(&rig, session.control(), 5)?;
    for (operation, command) in [
        (1, desktop_identity()?),
        (2, RecoveryCommand::Startup { total: body.len() as u32, offset: 0, bytes: &body }),
        (
            3,
            RecoveryCommand::CaptureRoot {
                identity: [4; 24],
                total: 2,
                offset: 0,
                units_le: &[65, 0],
            },
        ),
        (4, commit(key()?)?),
    ] {
        let frame =
            recovery_frame(&rig, &mut session, &boundary, operation, key()?, operation, command)?;
        owner.receive(&frame)?;
        drop(frame);
    }
    let (prepared, _) = owner.finish().map_err(|(e, _)| e)?;
    let effects = Arc::new(AtomicUsize::new(0));
    let original = prepared.origin.original_until();
    let mut spec = projection_spec(6, original)?;
    spec.reservation_bytes = PROCESS_CREATION_STORAGE_BYTES;
    let other = Rig::new()?;
    let mut input = Some(effects.clone());
    assert!(
        other
            .client
            .submit_recovery_desktop_creation(
                &other.scope,
                spec,
                &prepared.origin,
                &mut input,
                |value, _, _| Ok(Box::new(CreationDriver(value)))
            )
            .is_err()
    );
    assert!(input.as_ref().is_some_and(|value| Arc::ptr_eq(value, &effects)));
    let mut extended = spec;
    extended.deadline = Some(Moment::from_nanos(30_000_000_001));
    assert!(
        rig.client
            .submit_recovery_desktop_creation(
                &rig.scope,
                extended,
                &prepared.origin,
                &mut input,
                |value, _, _| Ok(Box::new(CreationDriver(value)))
            )
            .is_err()
    );
    assert!(input.is_some());
    assert_eq!(effects.load(Ordering::Acquire), 0);
    let mut creation = rig.client.submit_recovery_desktop_creation(
        &rig.scope,
        spec,
        &prepared.origin,
        &mut input,
        |value, _, _| Ok(Box::new(CreationDriver(value))),
    )?;
    assert!(input.is_none());
    let completed = rig.completion(&mut creation)?;
    assert!(matches!(completed.outcome, JobOutcome::Success(_)));
    assert_eq!(effects.load(Ordering::Acquire), 1);
    let mut repeated = Some(effects.clone());
    let mut next = spec;
    next.id = JobId::new(7)?;
    assert!(
        rig.client
            .submit_recovery_desktop_creation(
                &rig.scope,
                next,
                &prepared.origin,
                &mut repeated,
                |value, _, _| Ok(Box::new(CreationDriver(value)))
            )
            .is_err()
    );
    assert!(repeated.is_some());
    drop(completed);
    drop(creation);
    assert_eq!(rig.scope.snapshot().reserved_bytes, BYTES);
    let alias = prepared.origin.clone();
    drop(prepared);
    assert_eq!(rig.scope.snapshot().reserved_bytes, BYTES);
    session.control.close();
    assert!(alias.check().is_err());
    drop(alias);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    drop(rig.completion(&mut session.completion)?);
    drop(session);
    drop(boundary);
    close(&rig, &mut entry)
}

#[path = "recovery_startup_tests/enrollment.rs"]
mod enrollment;
