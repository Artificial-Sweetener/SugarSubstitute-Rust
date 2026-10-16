// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe capture and containment on the actual admitted custody worker and timer.
use super::*;
use crate::backend_custody::{
    BACKEND_CUSTODY_POOL_BYTES, BackendCustodyMemory, BackendCustodyPipeSession,
    BackendCustodyWorkerFailure, BackendCustodyWorkerReport,
};
const SECOND: u64 = 1_000_000_000;
struct Fixture {
    rig: Rig,
    scope: Scope,
    session: BackendCustodyPipeSession,
    memory: BackendCustodyMemory,
    worker: OwnedJob<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>,
    specification: JobSpec,
}
fn fixture() -> TestResult<Fixture> {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    authenticate(&rig, &mut session)?;
    let mut input = Some(session.control.take_worker_permit()?);
    let mut specification = spec(2, JobClass::NativeObservation, BACKEND_CUSTODY_POOL_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let start = rig.client.reserve_backend_custody_worker(&scope, specification, &mut input)?;
    assert!(input.is_none());
    let memory = start.metadata().clone();
    let ready = Arc::new(Gate::default());
    let observed = ready.clone();
    let worker = start
        .bind(move |_, _| {
            Ok(Box::new(ControlDriver { ready: observed })
                as Box<dyn crate::backend_custody::BackendCustodyDriver>)
        })
        .map_err(|_| "Control driver rejected")?;
    ready.wait()?;
    assert_eq!(memory.key(), Some(key()?));
    assert_eq!(memory.original_context().map(JobContext::job_id), Some(specification.id));
    memory.check_registration()?;
    assert_eq!(
        scope.snapshot().reserved_bytes,
        BACKEND_CUSTODY_POOL_BYTES + MIN_LIFECYCLE_PIPE_BYTES
    );
    Ok(Fixture { rig, scope, session, memory, worker, specification })
}
/// Join the genuine worker while its returned original claim remains observable.
fn finish(mut fixture: Fixture) -> TestResult {
    fixture.worker.cancel();
    let completion = fixture.rig.completion(&mut fixture.worker)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(completion.spec, fixture.specification);
    assert_eq!(completion.memory.bytes(), BACKEND_CUSTODY_POOL_BYTES);
    fixture.session.control.close();
    drop(terminal(&fixture.rig, &mut fixture.session)?);
    let core = fixture.rig.client.core.clone();
    let Fixture { rig, scope, session, memory, worker, specification: _ } = fixture;
    drop((session, memory, worker));
    let Rig { owner, client, clock, wake, scope: ordinary } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, BACKEND_CUSTODY_POOL_BYTES);
    assert_eq!(lock(&core.state).bytes, BACKEND_CUSTODY_POOL_BYTES);
    drop(completion);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(lock(&core.state).bytes, 0);
    drop((client, clock, wake, scope, ordinary));
    Ok(())
}
#[test]
fn recovery_capture_retains_sixty_seconds_then_one_original_fifteen_second_cleanup() -> TestResult {
    let fixture = fixture()?;
    let slot = fixture.memory.claim_slot()?;
    let turn = slot.begin_turn()?;
    let capture_until = Moment::from_nanos(60 * SECOND);
    let cleanup_until = Moment::from_nanos(75 * SECOND);
    let distant = Moment::from_nanos(200 * SECOND);
    assert_eq!(turn.begin_recovery_capture()?, capture_until);
    fixture.rig.clock.wait_until_registered(capture_until)?;
    assert!(turn.recovery_capture_pending()?);
    assert!(matches!(turn.begin_cleanup(distant), Err(AdmissionError::ShuttingDown)));
    fixture.rig.clock.advance(59 * SECOND);
    assert!(turn.recovery_capture_pending()?);
    assert_eq!(turn.begin_recovery_capture()?, capture_until);
    assert!(matches!(turn.begin_cleanup(distant), Err(AdmissionError::ShuttingDown)));
    assert!(slot.begin_turn().is_err());
    fixture.rig.clock.advance(SECOND);
    assert!(!turn.recovery_capture_pending()?);
    // Rejected timer transitions must not publish a cleanup proof for the next call.
    for _ in 0..2 {
        assert!(matches!(
            turn.begin_cleanup(capture_until),
            Err(AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired))
        ));
    }
    let proof = turn.begin_cleanup(distant)?;
    assert_eq!(proof.until(), cleanup_until);
    proof.check()?;
    fixture.rig.clock.wait_until_registered(cleanup_until)?;
    fixture.rig.clock.advance(14 * SECOND);
    assert_eq!(turn.begin_cleanup(distant)?.until(), cleanup_until);
    assert_eq!(turn.begin_recovery_capture()?, capture_until);
    proof.check()?;
    fixture.rig.clock.advance(SECOND);
    assert!(matches!(proof.check(), Err(AdmissionError::DeadlineExpired { deadline, now })
        if deadline == cleanup_until && now == cleanup_until));
    assert_eq!(turn.begin_cleanup(distant)?.until(), cleanup_until);
    drop(turn);
    assert!(slot.begin_turn().is_err());
    drop(proof);
    let ordinary = slot.begin_turn()?;
    let ordinary_until = Moment::from_nanos(90 * SECOND);
    let ordinary_proof = ordinary.begin_cleanup(distant)?;
    assert_eq!(ordinary_proof.until(), ordinary_until);
    assert!(matches!(ordinary.begin_recovery_capture(), Err(AdmissionError::ShuttingDown)));
    fixture.rig.clock.wait_until_registered(ordinary_until)?;
    fixture.rig.clock.advance(14 * SECOND);
    ordinary_proof.check()?;
    assert_eq!(ordinary.begin_cleanup(distant)?.until(), ordinary_until);
    fixture.rig.clock.advance(SECOND);
    assert!(
        matches!(ordinary_proof.check(), Err(AdmissionError::DeadlineExpired { deadline, now })
        if deadline == ordinary_until && now == ordinary_until)
    );
    drop((ordinary_proof, ordinary, slot));
    finish(fixture)
}
#[test]
fn late_recovery_capture_observation_cannot_renew_expired_containment() -> TestResult {
    let fixture = fixture()?;
    let slot = fixture.memory.claim_slot()?;
    let turn = slot.begin_turn()?;
    let capture_until = Moment::from_nanos(60 * SECOND);
    let cleanup_until = Moment::from_nanos(75 * SECOND);
    let distant = Moment::from_nanos(200 * SECOND);
    assert_eq!(turn.begin_recovery_capture()?, capture_until);
    fixture.rig.clock.wait_until_registered(capture_until)?;
    fixture.rig.clock.advance(80 * SECOND);
    assert!(!turn.recovery_capture_pending()?);
    let proof = turn.begin_cleanup(distant)?;
    assert_eq!(proof.until(), cleanup_until);
    assert!(matches!(proof.check(), Err(AdmissionError::DeadlineExpired { deadline, now })
        if deadline == cleanup_until && now == Moment::from_nanos(80 * SECOND)));
    assert_eq!(turn.begin_recovery_capture()?, capture_until);
    assert_eq!(turn.begin_cleanup(distant)?.until(), cleanup_until);
    fixture.rig.clock.advance(SECOND);
    assert!(matches!(turn.begin_cleanup(distant)?.check(),
        Err(AdmissionError::DeadlineExpired { deadline, now })
        if deadline == cleanup_until && now == Moment::from_nanos(81 * SECOND)));
    assert_eq!(
        fixture.scope.snapshot().reserved_bytes,
        BACKEND_CUSTODY_POOL_BYTES + MIN_LIFECYCLE_PIPE_BYTES
    );
    drop((proof, turn, slot));
    finish(fixture)
}
