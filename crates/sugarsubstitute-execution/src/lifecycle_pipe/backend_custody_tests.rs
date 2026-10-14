// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Issue custody permits through genuine admitted sessions and external boundary fakes.
use super::*;
use crate::backend_custody::{
    BackendCustodyPipeControl, BackendCustodyPipeObservation as Fact, BackendCustodyPipeReport,
    BackendCustodyPipeSession,
};
use sugarsubstitute_contracts::backend_custody::{BackendCustodyKey, BackendCustodySession};
#[cfg(windows)]
#[path = "captured_constructor_tests.rs"]
mod captured_constructor;
#[cfg(windows)]
#[path = "captured_purpose_tests.rs"]
mod captured_purpose;
#[path = "custodied_process_tests.rs"]
mod custodied_process;

/// Construct matching protocol facts without manufacturing a native permit.
fn key() -> TestResult<BackendCustodyKey> {
    Ok(BackendCustodyKey::new(run()?.owner, BackendCustodySession::new(9)?))
}
/// Admit the real production session owner while replacing only its native lease.
fn session(
    rig: &Rig,
    scope: &Scope,
    boundary: &Arc<Boundary>,
    parent: bool,
) -> TestResult<BackendCustodyPipeSession> {
    *lock(&boundary.binding) = Some(PeerBinding::BackendCustody(key()?));
    boundary.pending_read.store(false, Ordering::Release);
    let mut job = spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    job.operation = Operation::NativeLifecycle;
    let (inner, completion) = admit(
        &rig.client.core,
        scope,
        job,
        parent.then_some(PeerBinding::BackendCustody(key()?)),
        Moment::from_nanos(1000),
        Box::new(Lease(boundary.clone())),
        Owner::BackendCustody,
    )?;
    Ok(BackendCustodyPipeSession { control: BackendCustodyPipeControl { inner }, completion })
}
/// Await one actual observation using the owner's existing wake revision.
fn fact(
    rig: &Rig,
    session: &mut BackendCustodyPipeSession,
    select: impl Fn(&Fact) -> bool,
) -> TestResult<Fact> {
    loop {
        let revision = rig.wake.count();
        for value in session.drain(NonZeroUsize::MIN) {
            if select(&value) {
                return Ok(value);
            }
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        for value in session.drain(NonZeroUsize::MIN) {
            if select(&value) {
                return Ok(value);
            }
        }
        rig.wake.wait_after(revision)?;
    }
}
/// Observe the real terminal only after checked external retirement returns.
fn terminal(
    rig: &Rig,
    session: &mut BackendCustodyPipeSession,
) -> TestResult<JobCompletion<BackendCustodyPipeReport, LifecyclePipeFailure>> {
    loop {
        let revision = rig.wake.count();
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(value) = session.take_terminal() {
            return Ok(value);
        }
        rig.wake.wait_after(revision)?;
    }
}
/// Drive actual connection/read/capability ownership through the native boundary.
fn authenticate(rig: &Rig, session: &mut BackendCustodyPipeSession) -> TestResult {
    session.control.inner.bind_input(PeerInput::Simulated);
    drop(fact(rig, session, |value| matches!(value, Fact::Connected))?);
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900))?;
    drop(fact(rig, session, |value| matches!(value, Fact::Received { .. }))?);
    session.control.verify_capability([7; 32], Moment::from_nanos(900))?;
    let Fact::Authenticated(peer) =
        fact(rig, session, |value| matches!(value, Fact::Authenticated(_)))?
    else {
        return Err("Actual authenticated fact missing".into());
    };
    assert_eq!(peer.key(), key()?);
    Ok(())
}

#[test]
fn actual_child_permit_is_once_only_and_repeated_validation_keeps_session_live() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    assert!(session.control.take_worker_permit().is_err());
    authenticate(&rig, &mut session)?;
    let permit = session.control.take_worker_permit()?;
    assert_eq!(permit.key(), key()?);
    assert!(session.control.take_worker_permit().is_err());
    permit.validate_current_admission()?;
    permit.validate_current_admission()?;
    boundary.pending_read.store(true, Ordering::Release);
    session.control.read_passive(LifecyclePipeOperationId::new(2)?)?;
    drop(fact(&rig, &mut session, |value| matches!(value, Fact::ReadPending { .. }))?);
    assert!(session.take_terminal().is_none());
    session.control.close();
    assert!(permit.validate_current_admission().is_err());
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(boundary.lease_drops.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn actual_authenticated_parent_projection_never_mints_child_worker_permit() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    authenticate(&rig, &mut session)?;
    session.control.validate_current_admission()?;
    assert!(session.control.take_worker_permit().is_err());
    session.control.validate_current_admission()?;
    session.control.close();
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn original_scope_closed_without_cancel_rejects_retained_child_permit() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    authenticate(&rig, &mut session)?;
    let permit = session.control.take_worker_permit()?;
    rig.client.core.close_scope(scope.id());
    assert!(permit.validate_current_admission().is_err());
    assert!(session.control.validate_current_admission().is_err());
    session.control.close();
    assert!(matches!(terminal(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

/// Replace only native effects while retaining the actual admitted worker and pool.
struct CustodyBoundary {
    ready: Gate,
    wake: Mutex<Option<crate::backend_custody::BackendCustodyWake>>,
    initialized: AtomicUsize,
    retired: AtomicUsize,
    settled: std::sync::atomic::AtomicBool,
}
struct CustodyDriver {
    boundary: Arc<CustodyBoundary>,
    slots: Vec<sugarsubstitute_contracts::execution::MemoryClaim>,
}
impl crate::backend_custody::BackendCustodyDriver for CustodyDriver {
    fn initialize(
        &mut self,
        context: &JobContext,
        metadata: &crate::backend_custody::BackendCustodyMemory,
        wake: crate::backend_custody::BackendCustodyWake,
    ) -> Result<(), NativeObservationError> {
        assert_eq!(metadata.bytes(), 2 * 1024 * 1024);
        self.slots
            .try_reserve_exact(128)
            .map_err(|cause| NativeObservationError::new("test_slots", cause))?;
        for _ in 0..128 {
            self.slots.push(
                context
                    .memory()
                    .claim(512 * 1024)
                    .map_err(|cause| NativeObservationError::new("test_slot_claim", cause))?,
            );
        }
        assert!(context.memory().claim(1).is_err());
        *lock(&self.boundary.wake) = Some(wake.clone());
        self.boundary.initialized.fetch_add(1, Ordering::Release);
        self.boundary.ready.open();
        wake.observed();
        Ok(())
    }
    fn progress(
        &mut self,
        _context: &JobContext,
        _now: Moment,
    ) -> Result<crate::backend_custody::BackendCustodyProgress, NativeObservationError> {
        Ok(crate::backend_custody::BackendCustodyProgress::Pending)
    }
    fn retire(
        &mut self,
        _context: &JobContext,
        _now: Moment,
    ) -> Result<bool, NativeObservationError> {
        if !self.boundary.settled.load(Ordering::Acquire) {
            return Ok(false);
        }
        self.slots.clear();
        self.boundary.retired.fetch_add(1, Ordering::Release);
        Ok(true)
    }
}

#[test]
fn actual_custody_worker_retains_original_pool_until_checked_retirement_then_ack() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    authenticate(&rig, &mut session)?;
    let permit = session.control.take_worker_permit()?;
    let kernel = Arc::new(CustodyBoundary {
        ready: Gate::default(),
        wake: Mutex::new(None),
        initialized: AtomicUsize::new(0),
        retired: AtomicUsize::new(0),
        settled: std::sync::atomic::AtomicBool::new(false),
    });
    let _cleanup = CustodyCleanup(kernel.clone());
    let retained = kernel.clone();
    let mut worker_spec =
        spec(2, JobClass::NativeObservation, crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES)?;
    worker_spec.operation = Operation::NativeLifecycle;
    let mut input = Some(permit);
    let start = rig.client.reserve_backend_custody_worker(&scope, worker_spec, &mut input)?;
    assert!(input.is_none());
    let mut worker = start
        .bind(move |_, _| {
            Ok(Box::new(CustodyDriver { boundary: retained, slots: Vec::new() })
                as Box<dyn crate::backend_custody::BackendCustodyDriver>)
        })
        .map_err(|_| "Custody binding rejected")?;
    kernel.ready.wait()?;
    worker.cancel();
    assert!(worker.try_take_completion().is_none());
    assert_eq!(
        scope.snapshot().reserved_bytes,
        crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES + MIN_LIFECYCLE_PIPE_BYTES
    );
    kernel.settled.store(true, Ordering::Release);
    if let Some(wake) = &*lock(&kernel.wake) {
        wake.observed();
    }
    let completed = rig.completion(&mut worker)?;
    assert!(matches!(completed.outcome, JobOutcome::Cancelled));
    assert_eq!(kernel.retired.load(Ordering::Acquire), 1);
    drop(completed);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}

/// Release only this fake native gate before real execution-owner join on test unwind.
struct CustodyCleanup(Arc<CustodyBoundary>);
impl Drop for CustodyCleanup {
    fn drop(&mut self) {
        self.0.settled.store(true, Ordering::Release);
        if let Some(wake) = &*lock(&self.0.wake) {
            wake.observed();
        }
    }
}

#[test]
fn foreign_actual_child_permit_rejection_preserves_same_owner_and_never_constructs_driver()
-> TestResult {
    let rig = Rig::new()?;
    let foreign = Rig::new()?;
    let scope = scope(&rig)?;
    let destination = super::scope(&foreign)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    authenticate(&rig, &mut session)?;
    let permit = session.control.take_worker_permit()?;
    let pointer = std::ptr::from_ref(&*permit.shared);
    let mut job =
        spec(1, JobClass::NativeObservation, crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES)?;
    job.operation = Operation::NativeLifecycle;
    let mut input = Some(permit);
    let result = foreign.client.reserve_backend_custody_worker(&destination, job, &mut input);
    assert!(result.is_err());
    let retained = input.as_ref().ok_or("Original permit lost on rejection")?;
    assert_eq!(pointer, std::ptr::from_ref(&*retained.shared));
    retained.validate_current_admission()?;
    assert_eq!(destination.snapshot().reserved_bytes, 0);
    drop(input);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}

/// Native boundary has no outputs; all custody controls remain the genuine worker's.
struct ControlDriver {
    ready: Arc<Gate>,
}
impl crate::backend_custody::BackendCustodyDriver for ControlDriver {
    fn initialize(
        &mut self,
        _: &JobContext,
        _: &crate::backend_custody::BackendCustodyMemory,
        wake: crate::backend_custody::BackendCustodyWake,
    ) -> Result<(), NativeObservationError> {
        self.ready.open();
        wake.observed();
        Ok(())
    }
    fn progress(
        &mut self,
        _: &JobContext,
        _: Moment,
    ) -> Result<crate::backend_custody::BackendCustodyProgress, NativeObservationError> {
        Ok(crate::backend_custody::BackendCustodyProgress::Pending)
    }
    fn retire(&mut self, _: &JobContext, _: Moment) -> Result<bool, NativeObservationError> {
        Ok(true)
    }
}
#[test]
fn inert_source_construction_precedes_binding_and_is_once_only_on_real_queued_worker() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    authenticate(&rig, &mut session)?;
    let mut input = Some(session.control.take_worker_permit()?);
    let mut specification =
        spec(2, JobClass::NativeObservation, crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let start = rig.client.reserve_backend_custody_worker(&scope, specification, &mut input)?;
    let memory = start.metadata().clone();
    memory.begin_source()?;
    assert!(matches!(
        memory.begin_source(),
        Err(AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired))
    ));
    assert!(memory.check_registration().is_err());
    let ready = Arc::new(Gate::default());
    let observed = ready.clone();
    let mut worker = start
        .bind(move |_, _| {
            Ok(Box::new(ControlDriver { ready: observed })
                as Box<dyn crate::backend_custody::BackendCustodyDriver>)
        })
        .map_err(|_| "Control driver rejected")?;
    ready.wait()?;
    memory.check_registration()?;
    worker.cancel();
    drop(rig.completion(&mut worker)?);
    assert_eq!(
        scope.snapshot().reserved_bytes,
        crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES + MIN_LIFECYCLE_PIPE_BYTES
    );
    drop(memory);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}
#[test]
fn last_cleanup_alias_blocks_slot_reuse_and_each_new_turn_keeps_its_own_original_cutoff()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    authenticate(&rig, &mut session)?;
    let mut input = Some(session.control.take_worker_permit()?);
    let mut specification =
        spec(2, JobClass::NativeObservation, crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let start = rig.client.reserve_backend_custody_worker(&scope, specification, &mut input)?;
    let memory = start.metadata().clone();
    let ready = Arc::new(Gate::default());
    let observed = ready.clone();
    let mut worker = start
        .bind(move |_, _| {
            Ok(Box::new(ControlDriver { ready: observed })
                as Box<dyn crate::backend_custody::BackendCustodyDriver>)
        })
        .map_err(|_| "Control driver rejected")?;
    ready.wait()?;
    let slot = memory.claim_slot()?;
    let first = slot.begin_turn()?;
    let cleanup = first.begin_cleanup(Moment::from_nanos(900))?;
    assert_eq!(cleanup.until(), Moment::from_nanos(900));
    assert_eq!(first.begin_cleanup(Moment::from_nanos(10000))?.until(), cleanup.until());
    drop(first);
    assert!(slot.begin_turn().is_err());
    drop(cleanup);
    let next = slot.begin_turn()?;
    assert_eq!(next.begin_cleanup(Moment::from_nanos(800))?.until(), Moment::from_nanos(800));
    drop(next);
    drop(slot);
    worker.cancel();
    drop(rig.completion(&mut worker)?);
    drop(memory);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}
#[test]
fn contention_observation_uses_existing_timer_and_never_renews_original_cleanup() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, false)?;
    authenticate(&rig, &mut session)?;
    let mut input = Some(session.control.take_worker_permit()?);
    let mut specification =
        spec(2, JobClass::NativeObservation, crate::backend_custody::BACKEND_CUSTODY_POOL_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let start = rig.client.reserve_backend_custody_worker(&scope, specification, &mut input)?;
    let memory = start.metadata().clone();
    let ready = Arc::new(Gate::default());
    let observed = ready.clone();
    let mut worker = start
        .bind(move |_, _| {
            Ok(Box::new(ControlDriver { ready: observed })
                as Box<dyn crate::backend_custody::BackendCustodyDriver>)
        })
        .map_err(|_| "Control driver rejected")?;
    ready.wait()?;
    let slot = memory.claim_slot()?;
    let turn = slot.begin_turn()?;
    let until = Moment::from_nanos(500_000_000);
    let proof = turn.begin_cleanup(until)?;
    proof.observe_contention()?;
    assert!(!proof.contention_observed());
    rig.clock.wait_until_registered(Moment::from_nanos(250_000_000))?;
    rig.client.drain_completions(NonZeroUsize::MAX)?;
    rig.clock.advance(250_000_000);
    loop {
        let revision = rig.wake.count();
        if proof.contention_observed() {
            break;
        }
        rig.client.drain_completions(NonZeroUsize::MAX)?;
        if proof.contention_observed() {
            break;
        }
        rig.wake.wait_after(revision)?;
    }
    proof.check()?;
    assert_eq!(turn.begin_cleanup(Moment::from_nanos(9_000_000_000))?.until(), until);
    rig.clock.advance(250_000_000);
    assert!(
        matches!(proof.check(), Err(AdmissionError::DeadlineExpired { deadline, .. }) if deadline == until)
    );
    assert!(proof.observe_contention().is_err());
    drop(proof);
    drop(turn);
    drop(slot);
    worker.cancel();
    drop(rig.completion(&mut worker)?);
    drop(memory);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}

#[path = "backend_custody_cancel_tests.rs"]
mod cancel_start;
