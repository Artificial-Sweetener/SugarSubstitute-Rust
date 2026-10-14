// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep genuine backend capture registered and transferred without Desktop qualification authority.
use super::*;
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    crash::CaptureTarget,
    incident::IncidentSubject,
};
struct BackendInput(BackendCustodyKey);
impl InheritedCaptureLease for BackendInput {
    fn native_input(
        &self,
        cleanup: CleanupSource,
        issue: IssueSource,
        event: usize,
    ) -> Result<NativeCaptureInput, NativeObservationError> {
        let mut input = Input.native_input(cleanup, issue, event)?;
        let IncidentSubject::Supervisor(owner) = CaptureTarget::BackendCustody(self.0)
            .capture_subject()
            .map_err(|cause| NativeObservationError::new("backend_fixture_subject", cause))?
        else {
            return Err(NativeObservationError::new(
                "backend_fixture_subject",
                std::io::Error::other("Actual backend subject missing"),
            ));
        };
        input.subject = 2;
        input.namespace = *owner.namespace.bytes();
        input.epoch = *owner.epoch.bytes();
        input.instance = 0;
        input.generation = 0;
        Ok(input)
    }
    fn return_unissued(&mut self) -> Result<(), NativeObservationError> {
        Ok(())
    }
    fn retire(
        &mut self,
        original: &CaptureBundleRetirement,
    ) -> Result<bool, NativeObservationError> {
        original
            .check()
            .map_err(|cause| NativeObservationError::new("backend_fixture_retire", cause))?;
        Ok(true)
    }
}
fn backend()
-> TestResult<(RegisteredOwner, sugarsubstitute_contracts::lifecycle::RunKey, BackendCustodyKey)> {
    let desktop = input()?;
    let run = desktop.target().desktop().ok_or("Original Desktop fixture target missing")?;
    let key = BackendCustodyKey::new(run.owner, BackendCustodySession::new(1)?);
    let mut inherited = InheritedCaptureAttachInput::simulated(
        CaptureTarget::BackendCustody(key),
        desktop.original_deadline(),
        Box::new(BackendInput(key)),
    );
    assert!(inherited.reserve_preview_control_storage(8192).is_err());
    assert_eq!(inherited.preview_control_storage(), 0);
    assert_eq!(inherited.target(), CaptureTarget::BackendCustody(key));
    let mut owner = RegisteredOwner::with_input(None, inherited)?;
    owner.register()?;
    owner.retained()?;
    assert_eq!(owner.active.work.shared.cleanup.full_target, CaptureTarget::BackendCustody(key));
    assert_eq!(lock(&owner.active.work.shared.state).preview_control_bytes, 0);
    Ok((owner, run, key))
}
fn refuse_qualification(
    owner: &RegisteredOwner,
    run: sugarsubstitute_contracts::lifecycle::RunKey,
) -> TestResult<Scope> {
    let native = owner.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        Some(&owner.parent),
        true,
    )?;
    assert!(fixture_creator(owner, &native)?.is_none());
    let client = crate::ExecutionClient { core: owner.core.clone() };
    assert!(borrow_run_source(owner, &native, run).is_err());
    let spec = JobSpec {
        id: JobId::new(2)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(owner.active.work.shared.cleanup.attach_until),
        reservation_bytes: 4096,
        replacement: None,
    };
    assert!(
        client.submit_registered_desktop_fault(&owner.scope, spec, &owner.session, run).is_err()
    );
    let stage = lock(&owner.active.work.shared.state);
    assert!(stage.authenticated_creator.is_none());
    assert!(stage.fault_job.is_none());
    assert!(!stage.preview_loan_issued && stage.preview_job.is_none());
    drop(stage);
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    Ok(native)
}
#[test]
fn genuine_registered_backend_target_rejects_desktop_creator_fault_preview_and_funding()
-> TestResult {
    let (owner, run, _) = backend()?;
    let native = refuse_qualification(&owner, run)?;
    assert_eq!(owner.boundary.attaches.load(Ordering::Acquire), 1);
    owner.retained()?;
    drop(native);
    close(owner)
}
#[test]
fn genuine_backend_permanent_transfer_retains_original_target_and_cannot_arm_desktop_preview()
-> TestResult {
    let (mut owner, run, key) = backend()?;
    owner.session.control().request_cleanup();
    let crate::capture_attach::control::CaptureCleanupSnapshot::Latched { budget, .. } =
        owner.session.control().cleanup_snapshot()
    else {
        return Err("Actual original backend cleanup not latched".into());
    };
    owner.active.progress(&owner.core, budget.started, true);
    assert!(owner.active.completed());
    let mut permanent = owner
        .session
        .control()
        .try_take_process_lifetime()
        .ok_or("Actual backend permanent transfer missing")?
        .map_err(|cause| format!("{cause:?}"))?;
    {
        let state = lock(&owner.core.state);
        let association = state
            .jobs
            .get(&owner.active.context.job_id())
            .and_then(|record| record.process_capture.as_ref())
            .ok_or("Original backend association missing")?;
        assert_eq!(association.full_target, CaptureTarget::BackendCustody(key));
    }
    assert_eq!(permanent.reservation_bytes(), 4_194_304);
    let native = refuse_qualification(&owner, run)?;
    drop(native);
    drop(owner.active);
    drop(owner.metadata);
    drop(owner.queued_job);
    drop(owner.session);
    drop(lock(&owner.boundary.context).take());
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    // Only the fake external capsule is destroyed; no native process-exit proof is manufactured.
    let capsule = permanent.capsule.take().ok_or("Original fake backend capsule missing")?;
    drop(permanent);
    drop(capsule);
    assert_eq!(owner.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
