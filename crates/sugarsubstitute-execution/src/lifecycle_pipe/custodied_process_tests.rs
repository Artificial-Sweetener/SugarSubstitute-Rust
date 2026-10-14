// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real finite creation owner while replacing only native outputs and ACKs.
use super::*;
use crate::backend_custody::process::{
    CUSTODIED_PROCESS_STORAGE_BYTES, CustodiedProcessDriver, CustodiedProcessMemory,
    CustodiedProcessProgress, CustodiedProcessWake,
};

/// Native output and peer ACKs remain independent controlled boundary observations.
struct CreationBoundary {
    prepared: Gate,
    suspended: Gate,
    retiring: Gate,
    acknowledged: AtomicUsize,
    resumed: AtomicUsize,
    closed: AtomicBool,
    wake: Mutex<Option<CustodiedProcessWake>>,
    memory: Mutex<Option<CustodiedProcessMemory>>,
}
impl CreationBoundary {
    /// Create inert external cells; the execution owner installs its actual wake later.
    fn new() -> Arc<Self> {
        Arc::new(Self {
            prepared: Gate::default(),
            suspended: Gate::default(),
            retiring: Gate::default(),
            acknowledged: AtomicUsize::new(0),
            resumed: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            wake: Mutex::new(None),
            memory: Mutex::new(None),
        })
    }
    /// Deliver a controlled peer observation through the original execution wake.
    fn observe(&self, acknowledgements: usize, closed: bool) -> TestResult {
        self.acknowledged.store(acknowledgements, Ordering::Release);
        self.closed.store(closed, Ordering::Release);
        lock(&self.wake).as_ref().ok_or("Original creation wake absent")?.observed();
        Ok(())
    }
}
struct CreationDriver {
    boundary: Arc<CreationBoundary>,
    phase: usize,
}
impl CustodiedProcessDriver for CreationDriver {
    fn initialize(
        &mut self,
        context: &JobContext,
        memory: &CustodiedProcessMemory,
        wake: CustodiedProcessWake,
    ) -> Result<(), NativeObservationError> {
        assert_eq!(context.spec().class, JobClass::InfrastructureLifecycle);
        assert_eq!(memory.bytes(), CUSTODIED_PROCESS_STORAGE_BYTES);
        assert!(context.memory().claim(1).is_err());
        memory
            .check_effect()
            .map_err(|cause| NativeObservationError::new("test_creation", cause))?;
        *lock(&self.boundary.memory) = Some(memory.clone());
        *lock(&self.boundary.wake) = Some(wake);
        self.boundary.prepared.open();
        Ok(())
    }
    fn progress(
        &mut self,
        _context: &JobContext,
        _now: Moment,
    ) -> Result<CustodiedProcessProgress, NativeObservationError> {
        let acknowledged = self.boundary.acknowledged.load(Ordering::Acquire);
        if self.phase == 0 && acknowledged >= 1 {
            self.phase = 1;
            self.boundary.suspended.open();
        }
        if self.phase == 1 && acknowledged >= 2 {
            self.phase = 2;
            self.boundary.resumed.fetch_add(1, Ordering::Release);
            return Ok(CustodiedProcessProgress::Started);
        }
        Ok(CustodiedProcessProgress::Pending)
    }
    fn retire(
        &mut self,
        _context: &JobContext,
        _now: Moment,
    ) -> Result<bool, NativeObservationError> {
        self.boundary.retiring.open();
        if self.phase == 2 {
            return Ok(true);
        }
        let memory = lock(&self.boundary.memory).as_ref().cloned().ok_or_else(|| {
            NativeObservationError::new(
                "test_creation_owner",
                std::io::Error::other("Missing original storage"),
            )
        })?;
        let cleanup = memory
            .begin_retirement()
            .map_err(|cause| NativeObservationError::new("test_creation_retirement", cause))?;
        cleanup
            .check()
            .map_err(|cause| NativeObservationError::new("test_creation_retirement", cause))?;
        Ok(self.boundary.closed.load(Ordering::Acquire))
    }
}

/// Admit the same finite job under a genuine authenticated original parent allocation.
fn creation(
    rig: &Rig,
    scope: &Scope,
    parent: &crate::backend_custody::BackendCustodyParentStorage,
    boundary: &Arc<CreationBoundary>,
) -> TestResult<
    OwnedJob<
        crate::backend_custody::process::CustodiedProcessReport,
        crate::backend_custody::process::CustodiedProcessFailure,
    >,
> {
    let mut job = spec(2, JobClass::InfrastructureLifecycle, CUSTODIED_PROCESS_STORAGE_BYTES)?;
    job.operation = Operation::NativeLifecycle;
    job.deadline = Some(Moment::from_nanos(800));
    let mut input = Some(boundary.clone());
    Ok(rig.client.submit_custodied_process(scope, job, parent, &mut input, |boundary, _, _| {
        Ok(Box::new(CreationDriver { boundary, phase: 0 }))
    })?)
}

#[test]
fn actual_finite_owner_requires_both_acknowledgements_before_resume_and_terminal() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let pipe = Boundary::new();
    let mut session = session(&rig, &scope, &pipe, true)?;
    authenticate(&rig, &mut session)?;
    let parent = session.control.take_parent_storage()?;
    let boundary = CreationBoundary::new();
    let creation_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut job = creation(&rig, &creation_scope, &parent, &boundary)?;
    boundary.prepared.wait()?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(boundary.resumed.load(Ordering::Acquire), 0);
    boundary.observe(1, false)?;
    boundary.suspended.wait()?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(boundary.resumed.load(Ordering::Acquire), 0);
    boundary.observe(2, false)?;
    let done = rig.completion(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(_)));
    assert_eq!(boundary.resumed.load(Ordering::Acquire), 1);
    assert_eq!(done.spec.deadline, Some(Moment::from_nanos(800)));
    drop(done);
    assert_eq!(creation_scope.snapshot().reserved_bytes, CUSTODIED_PROCESS_STORAGE_BYTES);
    drop(lock(&boundary.wake).take());
    drop(lock(&boundary.memory).take());
    drop(parent);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    drop(session);
    // Terminal delivery precedes destruction of the execution worker's Active owner.
    drop(rig.owner);
    assert_eq!(creation_scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn cancellation_with_root_ack_pending_keeps_original_claim_until_checked_close_and_last_alias()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let pipe = Boundary::new();
    let mut session = session(&rig, &scope, &pipe, true)?;
    authenticate(&rig, &mut session)?;
    let parent = session.control.take_parent_storage()?;
    let boundary = CreationBoundary::new();
    let creation_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut job = creation(&rig, &creation_scope, &parent, &boundary)?;
    boundary.prepared.wait()?;
    boundary.observe(1, false)?;
    boundary.suspended.wait()?;
    job.cancel();
    boundary.retiring.wait()?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(boundary.resumed.load(Ordering::Acquire), 0);
    let memory = lock(&boundary.memory).as_ref().cloned().ok_or("Original family claim absent")?;
    let original = memory.begin_retirement()?;
    rig.clock.advance(1);
    assert_eq!(memory.begin_retirement()?.until(), original.until());
    boundary.observe(2, true)?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.resumed.load(Ordering::Acquire), 0);
    drop(lock(&boundary.wake).take());
    drop(lock(&boundary.memory).take());
    drop(memory);
    drop(parent);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    drop(session);
    drop(rig.owner);
    assert_eq!(creation_scope.snapshot().reserved_bytes, CUSTODIED_PROCESS_STORAGE_BYTES);
    drop(original);
    assert_eq!(creation_scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn original_finite_deadline_rejects_late_root_ack_without_resuming() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let pipe = Boundary::new();
    let mut session = session(&rig, &scope, &pipe, true)?;
    authenticate(&rig, &mut session)?;
    let parent = session.control.take_parent_storage()?;
    let boundary = CreationBoundary::new();
    let creation_scope =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut job = creation(&rig, &creation_scope, &parent, &boundary)?;
    boundary.prepared.wait()?;
    boundary.observe(1, false)?;
    boundary.suspended.wait()?;
    rig.clock.advance(800);
    boundary.observe(2, true)?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.resumed.load(Ordering::Acquire), 0);
    drop(lock(&boundary.wake).take());
    drop(lock(&boundary.memory).take());
    drop(parent);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}

#[test]
fn wrong_finite_spec_rejects_before_input_transfer_factory_or_capacity() -> TestResult {
    let rig = Rig::new()?;
    let native = scope(&rig)?;
    let pipe = Boundary::new();
    let mut session = session(&rig, &native, &pipe, true)?;
    authenticate(&rig, &mut session)?;
    let parent = session.control.take_parent_storage()?;
    let ordinary =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let boundary = CreationBoundary::new();
    let mut input = Some(boundary.clone());
    let mut job = spec(2, JobClass::InfrastructureLifecycle, CUSTODIED_PROCESS_STORAGE_BYTES)?;
    job.operation = Operation::NativeLifecycle;
    let rejected = rig.client.submit_custodied_process(
        &ordinary,
        job,
        &parent,
        &mut input,
        |_, _, _| -> Result<Box<dyn CustodiedProcessDriver>, NativeObservationError> {
            Err(NativeObservationError::new(
                "unexpected_factory",
                std::io::Error::other("Rejected factory ran"),
            ))
        },
    );
    assert!(matches!(rejected, Err(AdmissionError::DeadlineRequired)));
    assert!(input.as_ref().is_some_and(|retained| Arc::ptr_eq(retained, &boundary)));
    assert_eq!(ordinary.snapshot().reserved_bytes, 0);
    assert!(lock(&boundary.memory).is_none());
    drop(parent);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}

#[test]
fn foreign_actual_parent_source_rejects_without_moving_input_or_constructing_driver() -> TestResult
{
    let rig = Rig::new()?;
    let native = scope(&rig)?;
    let pipe = Boundary::new();
    let mut session = session(&rig, &native, &pipe, true)?;
    authenticate(&rig, &mut session)?;
    let parent = session.control.take_parent_storage()?;
    let destination = Rig::new()?;
    let ordinary = destination
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let boundary = CreationBoundary::new();
    let mut input = Some(boundary.clone());
    let mut job = spec(2, JobClass::InfrastructureLifecycle, CUSTODIED_PROCESS_STORAGE_BYTES)?;
    job.operation = Operation::NativeLifecycle;
    job.deadline = Some(Moment::from_nanos(800));
    let rejected = destination.client.submit_custodied_process(
        &ordinary,
        job,
        &parent,
        &mut input,
        |_, _, _| -> Result<Box<dyn CustodiedProcessDriver>, NativeObservationError> {
            Err(NativeObservationError::new(
                "unexpected_factory",
                std::io::Error::other("Foreign factory ran"),
            ))
        },
    );
    assert!(matches!(
        rejected,
        Err(AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired))
    ));
    assert!(input.as_ref().is_some_and(|retained| Arc::ptr_eq(retained, &boundary)));
    assert_eq!(ordinary.snapshot().reserved_bytes, 0);
    assert!(lock(&boundary.memory).is_none());
    parent.check()?;
    drop(parent);
    session.control.close();
    drop(terminal(&rig, &mut session)?);
    Ok(())
}
