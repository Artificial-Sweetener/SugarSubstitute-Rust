// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject counterfeit native metadata through actual registered capture and Core admission.
use super::*;

fn fault_spec(owner: &RegisteredOwner, id: u64) -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(owner.active.work.shared.cleanup.attach_until),
        reservation_bytes: 4096,
        replacement: None,
    })
}
fn release(owner: RegisteredOwner) -> TestResult {
    owner.session.control().request_cleanup();
    owner.settle()
}

#[test]
fn matching_native_reply_cannot_mint_authenticated_creator_or_consume_job_identity() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let spec = fault_spec(&owner, 2)?;
    let run = owner
        .active
        .work
        .shared
        .cleanup
        .full_target
        .desktop()
        .ok_or("Actual Desktop target missing")?;
    let before = owner.scope.snapshot().reserved_bytes;
    for _ in 0..2 {
        assert!(matches!(
            client.submit_registered_desktop_fault(&owner.scope, spec, &owner.session, run),
            Err(AdmissionError::InvalidCaptureAdmission)
        ));
        assert_eq!(owner.scope.snapshot().reserved_bytes, before);
        let state = lock(&owner.core.state);
        assert!(!state.jobs.contains_key(&spec.id));
        assert!(state.queue.is_empty());
        let stage = lock(&owner.active.work.shared.state);
        assert!(stage.authenticated_creator.is_none());
        assert!(stage.fault_job.is_none());
        assert!(stage.accepted_stage.is_none());
    }
    release(owner)
}

#[test]
fn foreign_core_and_original_scope_cancellation_reject_before_fault_effect() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let spec = fault_spec(&owner, 2)?;
    let run = owner
        .active
        .work
        .shared
        .cleanup
        .full_target
        .desktop()
        .ok_or("Actual Desktop target missing")?;
    let foreign = crate::ExecutionClient {
        core: Core::new(
            ExecutionId::new(2)?,
            Arc::new(ClockPort(owner.clock.clone())),
            Arc::new(Wake::default()),
            crate::runtime::DEFAULT_MEMORY_BYTES,
        ),
    };
    assert!(
        foreign.submit_registered_desktop_fault(&owner.scope, spec, &owner.session, run).is_err()
    );
    let client = crate::ExecutionClient { core: owner.core.clone() };
    owner.parent.cancel();
    assert!(matches!(
        client.submit_registered_desktop_fault(&owner.scope, spec, &owner.session, run),
        Err(AdmissionError::ScopeClosed { .. })
    ));
    assert!(lock(&owner.active.work.shared.state).fault_job.is_none());
    release(owner)
}

#[test]
fn expired_original_registration_cutoff_cannot_be_replaced_for_fault() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let spec = fault_spec(&owner, 2)?;
    let run = owner
        .active
        .work
        .shared
        .cleanup
        .full_target
        .desktop()
        .ok_or("Actual Desktop target missing")?;
    owner.clock.advance(spec.deadline.ok_or("Original cutoff missing")?.as_nanos());
    assert!(matches!(
        client.submit_registered_desktop_fault(&owner.scope, spec, &owner.session, run),
        Err(AdmissionError::DeadlineExpired { .. })
    ));
    let renewed = JobSpec { deadline: Some(Moment::from_nanos(u64::MAX)), ..spec };
    assert!(matches!(
        client.submit_registered_desktop_fault(&owner.scope, renewed, &owner.session, run),
        Err(AdmissionError::InvalidCaptureAdmission)
    ));
    assert!(lock(&owner.active.work.shared.state).fault_job.is_none());
    release(owner)
}

#[test]
fn wrong_budget_class_scope_and_cleanup_source_never_admit_fault() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let original = fault_spec(&owner, 2)?;
    let run = owner
        .active
        .work
        .shared
        .cleanup
        .full_target
        .desktop()
        .ok_or("Actual Desktop target missing")?;
    for spec in [
        JobSpec { reservation_bytes: 4097, ..original },
        JobSpec { class: JobClass::InfrastructureOrdinary, ..original },
        JobSpec { deadline: None, ..original },
    ] {
        assert!(
            client
                .submit_registered_desktop_fault(&owner.scope, spec, &owner.session, run)
                .is_err()
        );
    }
    assert!(
        client
            .submit_registered_desktop_fault(&owner.parent, original, &owner.session, run)
            .is_err()
    );
    owner.session.control().request_cleanup();
    assert!(matches!(
        client.submit_registered_desktop_fault(&owner.scope, original, &owner.session, run),
        Err(AdmissionError::CaptureRetiring)
    ));
    assert!(lock(&owner.active.work.shared.state).fault_job.is_none());
    owner.settle()
}

#[test]
fn original_clock_observation_can_cancel_before_any_fault_claim_or_issuance() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let spec = fault_spec(&owner, 2)?;
    let run = owner
        .active
        .work
        .shared
        .cleanup
        .full_target
        .desktop()
        .ok_or("Actual Desktop target missing")?;
    let core = owner.core.clone();
    let parent = owner.parent.id();
    owner.clock.on_next_current_thread_observation(Box::new(move || core.close_scope(parent)));
    let client = crate::ExecutionClient { core: owner.core.clone() };
    assert!(matches!(
        client.submit_registered_desktop_fault(&owner.scope, spec, &owner.session, run),
        Err(AdmissionError::ScopeClosed { .. })
    ));
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    assert!(lock(&owner.active.work.shared.state).fault_job.is_none());
    release(owner)
}

#[test]
fn matching_execution_scope_job_run_numbers_do_not_join_foreign_capture_ledgers() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    let mut foreign = RegisteredOwner::new(None)?;
    owner.register()?;
    foreign.register()?;
    let spec = fault_spec(&owner, 2)?;
    let run = owner
        .active
        .work
        .shared
        .cleanup
        .full_target
        .desktop()
        .ok_or("Actual Desktop target missing")?;
    assert_eq!(owner.scope.owner_id(), foreign.scope.owner_id());
    assert_eq!(owner.scope.id(), foreign.scope.id());
    assert_eq!(owner.active.work.shared.cleanup.job, foreign.active.work.shared.cleanup.job);
    let client = crate::ExecutionClient { core: foreign.core.clone() };
    assert!(matches!(
        client.submit_registered_desktop_fault(&foreign.scope, spec, &owner.session, run),
        Err(AdmissionError::InvalidCaptureAdmission)
    ));
    assert!(lock(&owner.active.work.shared.state).fault_job.is_none());
    assert!(lock(&foreign.active.work.shared.state).fault_job.is_none());
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    assert_eq!(foreign.scope.snapshot().reserved_bytes, 4_194_304);
    release(owner)?;
    release(foreign)
}
