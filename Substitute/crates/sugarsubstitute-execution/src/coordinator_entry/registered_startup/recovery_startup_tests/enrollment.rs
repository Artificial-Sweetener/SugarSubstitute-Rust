// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require genuine received enrollment and actual running creation memory before resume permission.
use super::*;
use crate::{lifecycle_pipe::tests::RecoveryChildBoundary, runtime_test_support::Gate};
use std::sync::Mutex;
use sugarsubstitute_contracts::recovery_supervisor::RecoveryProcessIdentity;

fn receive_startup(
    rig: &Rig,
    session: &mut RecoveryPipeSession,
    boundary: &RecoveryChildBoundary,
    body: &[u8],
    assembly_id: u64,
) -> TestResult<RecoveryStartup> {
    let mut owner = assembly(rig, session.control(), assembly_id)?;
    for (sequence, command) in [
        (1, desktop_identity()?),
        (2, RecoveryCommand::Startup { total: body.len() as u32, offset: 0, bytes: body }),
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
        let frame = recovery_frame(rig, session, boundary, sequence, key()?, sequence, command)?;
        owner.receive(&frame)?;
        drop(frame);
    }
    let (prepared, sequence) = owner.finish().map_err(|(cause, _)| cause)?;
    assert_eq!(sequence, 4);
    assert_eq!(prepared.origin.desktop_identity()?.run, key()?.run());
    Ok(prepared)
}
/// No native outputs exist at this external boundary; the central creation owner remains real.
struct PendingCreation {
    ready: Arc<Gate>,
    memory: Arc<Mutex<Option<ProcessCreationMemory>>>,
}
impl ProcessCreationDriver for PendingCreation {
    fn initialize(
        &mut self,
        _: &JobContext,
        memory: &ProcessCreationMemory,
        _: ProcessCreationWake,
    ) -> Result<(), crate::native::NativeObservationError> {
        memory.check_effect().map_err(|cause| {
            crate::native::NativeObservationError::new("enrollment_fixture", cause)
        })?;
        *lock(&self.memory) = Some(memory.clone());
        self.ready.open();
        Ok(())
    }
    fn progress(
        &mut self,
        _: &JobContext,
        _: Moment,
    ) -> Result<ProcessCreationProgress, crate::native::NativeObservationError> {
        Ok(ProcessCreationProgress::Pending)
    }
    fn retire(
        &mut self,
        _: &JobContext,
        _: Moment,
    ) -> Result<bool, crate::native::NativeObservationError> {
        Ok(true)
    }
}
fn running_creation(
    rig: &Rig,
    origin: &AuthenticatedRecoveryOrigin,
    id: u64,
) -> TestResult<(OwnedJob<ProcessCreationReport, ProcessCreationFailure>, ProcessCreationMemory)> {
    let ready = Arc::new(Gate::default());
    let memory = Arc::new(Mutex::new(None));
    let mut input = Some(PendingCreation { ready: ready.clone(), memory: memory.clone() });
    let mut specification = projection_spec(id, origin.original_until())?;
    specification.reservation_bytes = PROCESS_CREATION_STORAGE_BYTES;
    let job = rig.client.submit_recovery_desktop_creation(
        &rig.scope,
        specification,
        origin,
        &mut input,
        |driver, _, _| Ok(Box::new(driver)),
    )?;
    assert!(input.is_none());
    ready.wait()?;
    let memory = lock(&memory).take().ok_or("Running creation did not retain original memory")?;
    memory.check_effect()?;
    assert_eq!(memory.original_context()?.job_id(), specification.id);
    Ok((job, memory))
}
fn final_credit(
    rig: Rig,
    completions: Vec<JobCompletion<ProcessCreationReport, ProcessCreationFailure>>,
) -> TestResult {
    let expected = completions.len() as u64 * PROCESS_CREATION_STORAGE_BYTES;
    for completion in &completions {
        assert!(matches!(completion.outcome, JobOutcome::Cancelled));
        assert_eq!(completion.memory.bytes(), PROCESS_CREATION_STORAGE_BYTES);
    }
    let core = rig.client.core.clone();
    let Rig { owner, client, clock, wake, scope } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, expected);
    assert_eq!(lock(&core.state).bytes, expected);
    drop(completions);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(lock(&core.state).bytes, 0);
    drop((client, clock, wake, scope));
    Ok(())
}
#[test]
fn recovery_enrollment_rejects_unadmitted_creation_foreign_frame_wrong_identity_and_sequence_without_consuming_token()
-> TestResult {
    let rig = Rig::new()?;
    let (_entry_scope, mut entry, body) = launch_body(&rig)?;
    let (_scope, mut session, boundary) = recovery_child(&rig, 3, 4, key()?)?;
    let prepared = receive_startup(&rig, &mut session, &boundary, &body, 5)?;
    let expected = RecoveryProcessIdentity::new(101, 201)?;
    let premature = recovery_frame(
        &rig,
        &mut session,
        &boundary,
        5,
        key()?,
        5,
        RecoveryCommand::DesktopEnrolled(expected),
    )?;
    assert!(matches!(
        prepared.origin.admit_desktop_enrollment(&premature, expected),
        Err(RecoveryStartupError::Binding)
    ));
    drop(premature);
    let (mut creation, memory) = running_creation(&rig, &prepared.origin, 6)?;
    let (_foreign_scope, mut foreign, foreign_boundary) = recovery_child(&rig, 4, 8, key()?)?;
    let foreign_prepared = receive_startup(&rig, &mut foreign, &foreign_boundary, &body, 9)?;
    let (mut foreign_creation, foreign_memory) =
        running_creation(&rig, &foreign_prepared.origin, 10)?;
    let foreign_frame = recovery_frame(
        &rig,
        &mut foreign,
        &foreign_boundary,
        5,
        key()?,
        5,
        RecoveryCommand::DesktopEnrolled(expected),
    )?;
    assert!(matches!(
        prepared.origin.admit_desktop_enrollment(&foreign_frame, expected),
        Err(RecoveryStartupError::Binding)
    ));
    drop(foreign_frame);
    let wrong_birth = RecoveryProcessIdentity::new(expected.pid(), expected.birth() + 1)?;
    let wrong_key = RecoveryGrantKey::new(
        sugarsubstitute_contracts::recovery_supervisor::RecoveryGrantId::new(2)?,
        key()?.previous(),
        key()?.report(),
    )?;
    for (operation, frame_key, sequence, identity) in [
        (6, key()?, 4, expected),
        (7, key()?, 6, expected),
        (8, key()?, 5, wrong_birth),
        (9, wrong_key, 5, expected),
    ] {
        let frame = recovery_frame(
            &rig,
            &mut session,
            &boundary,
            operation,
            frame_key,
            sequence,
            RecoveryCommand::DesktopEnrolled(identity),
        )?;
        assert!(matches!(
            prepared.origin.admit_desktop_enrollment(&frame, expected),
            Err(RecoveryStartupError::Order)
        ));
        drop(frame);
    }
    let enrolled = recovery_frame(
        &rig,
        &mut session,
        &boundary,
        10,
        key()?,
        5,
        RecoveryCommand::DesktopEnrolled(expected),
    )?;
    let token = prepared.origin.admit_desktop_enrollment(&enrolled, expected)?;
    assert!(matches!(
        prepared.origin.admit_desktop_enrollment(&enrolled, expected),
        Err(RecoveryStartupError::Order)
    ));
    drop(enrolled);
    token.check(&prepared.origin, expected, &memory)?;
    assert!(token.check(&prepared.origin, wrong_birth, &memory).is_err());
    assert!(token.check(&foreign_prepared.origin, expected, &memory).is_err());
    assert!(token.check(&prepared.origin, expected, &foreign_memory).is_err());
    creation.cancel();
    assert!(token.check(&prepared.origin, expected, &memory).is_err());
    let completed = rig.completion(&mut creation)?;
    foreign_creation.cancel();
    let foreign_completed = rig.completion(&mut foreign_creation)?;
    session.control.close();
    foreign.control.close();
    drop((token, memory, foreign_memory, prepared, foreign_prepared, creation, foreign_creation));
    drop(rig.completion(&mut session.completion)?);
    drop(rig.completion(&mut foreign.completion)?);
    drop((session, foreign, boundary, foreign_boundary));
    close(&rig, &mut entry)?;
    drop(entry);
    final_credit(rig, vec![completed, foreign_completed])
}
#[test]
fn recovery_enrollment_original_cancellation_and_expiry_revoke_received_token_and_running_memory()
-> TestResult {
    for expired in [false, true] {
        let rig = Rig::new()?;
        let (_entry_scope, mut entry, body) = launch_body(&rig)?;
        let (_scope, mut session, boundary) = recovery_child(&rig, 3, 4, key()?)?;
        let prepared = receive_startup(&rig, &mut session, &boundary, &body, 5)?;
        let (mut creation, memory) = running_creation(&rig, &prepared.origin, 6)?;
        let expected = RecoveryProcessIdentity::new(101, 201)?;
        let frame = recovery_frame(
            &rig,
            &mut session,
            &boundary,
            5,
            key()?,
            5,
            RecoveryCommand::DesktopEnrolled(expected),
        )?;
        let token = prepared.origin.admit_desktop_enrollment(&frame, expected)?;
        token.check(&prepared.origin, expected, &memory)?;
        if expired {
            rig.clock.advance(30_000_000_000);
        } else {
            session.control.close();
        }
        assert!(prepared.origin.check().is_err());
        assert!(token.check(&prepared.origin, expected, &memory).is_err());
        assert!(matches!(
            prepared.origin.admit_desktop_enrollment(&frame, expected),
            Err(RecoveryStartupError::Admission(_))
        ));
        assert!(memory.check_effect().is_err());
        drop((frame, token, prepared, memory));
        if !expired {
            creation.cancel();
        }
        let completed = rig.completion(&mut creation)?;
        if expired {
            assert!(matches!(completed.outcome, JobOutcome::TimedOut));
        } else {
            assert!(matches!(completed.outcome, JobOutcome::Cancelled));
        }
        session.control.close();
        drop(rig.completion(&mut session.completion)?);
        drop((session, boundary, creation));
        close(&rig, &mut entry)?;
        drop(entry);
        let core = rig.client.core.clone();
        let Rig { owner, client, clock, wake, scope } = rig;
        drop(owner);
        assert_eq!(scope.snapshot().reserved_bytes, PROCESS_CREATION_STORAGE_BYTES);
        assert_eq!(lock(&core.state).bytes, PROCESS_CREATION_STORAGE_BYTES);
        drop(completed);
        assert_eq!(scope.snapshot().reserved_bytes, 0);
        assert_eq!(lock(&core.state).bytes, 0);
        drop((client, clock, wake, scope));
    }
    Ok(())
}
