// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve current/permanent source identity and original deadline at actual worker entry.
use super::*;

#[test]
fn preview_genuine_permanent_transfer_matches_same_original_capsule_and_rejects_changed_identity()
-> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65536)?;
    owner.session.control().request_cleanup();
    let crate::capture_attach::control::CaptureCleanupSnapshot::Latched { budget, .. } =
        owner.session.control().cleanup_snapshot()
    else {
        return Err("Actual original cleanup not latched".into());
    };
    owner.active.progress(&owner.core, budget.started, true);
    assert!(owner.active.completed());
    let mut permanent = owner
        .session
        .control()
        .try_take_process_lifetime()
        .ok_or("Actual permanent transfer missing")?
        .map_err(|cause| format!("{cause:?}"))?;
    let custody = owner.active.work.shared.clone();
    assert_eq!(lock(&custody.state).phase, control::DesktopCapturePhase::Transferred);
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let mut job = worker(&owner, spec, move |context| {
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:\bounded-preview.png"));
        let (job, original_target) = {
            let mut state = lock(&client.core.state);
            let record =
                state.jobs.get_mut(&custody.cleanup.job).ok_or(PreviewArmingError::Destination)?;
            let association =
                record.process_capture.as_mut().ok_or(PreviewArmingError::Destination)?;
            let original_target = association.full_target;
            association.full_target =
                sugarsubstitute_contracts::crash::CaptureTarget::BackendCustody(
                    sugarsubstitute_contracts::backend_custody::BackendCustodyKey::new(
                        identity.run.owner,
                        sugarsubstitute_contracts::backend_custody::BackendCustodySession::new(1)
                            .map_err(|_| PreviewArmingError::Destination)?,
                    ),
                );
            (custody.cleanup.job, original_target)
        };
        assert!(client.bind_preview_source(context, &mut loan, &mut path, identity).is_err());
        assert!(loan.is_some() && path.is_some());
        {
            let mut state = lock(&client.core.state);
            let association = state
                .jobs
                .get_mut(&job)
                .and_then(|record| record.process_capture.as_mut())
                .ok_or(PreviewArmingError::Destination)?;
            association.full_target = original_target;
        }
        let original =
            lock(&custody.state).permanent_identity.ok_or(PreviewArmingError::Destination)?;
        lock(&custody.state).permanent_identity = Some(original.wrapping_add(1));
        assert!(client.bind_preview_source(context, &mut loan, &mut path, identity).is_err());
        assert!(loan.is_some() && path.is_some());
        lock(&custody.state).permanent_identity = Some(original);
        let seal = client.bind_preview_source(context, &mut loan, &mut path, identity)?;
        seal.check(context, identity)?;
        Ok::<_, PreviewArmingError>(seal)
    })?;
    drop(job.try_take_completion().ok_or("Actual preview terminal missing")?);
    drop(job);
    drop(native);
    drop(owner.active);
    drop(owner.metadata);
    drop(owner.queued_job);
    drop(owner.session);
    drop(lock(&owner.boundary.context).take());
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    let capsule = permanent.capsule.take().ok_or("Original fake capsule missing")?;
    drop(permanent);
    drop(capsule);
    assert_eq!(owner.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn preview_actual_capture_cleanup_rejects_fresh_worker_effects_and_keeps_original_inputs()
-> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65536)?;
    let custody = owner.active.work.shared.clone();
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let mut job = worker(&owner, spec, move |context| {
        custody.request(
            crate::task::Stop::Cancelled,
            crate::capture_attach::control::CleanupTrigger::Released,
        );
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:\bounded-preview.png"));
        assert!(matches!(
            client.bind_preview_source(context, &mut loan, &mut path, identity),
            Err(PreviewArmingError::Admission(AdmissionError::CaptureRetiring))
        ));
        assert!(loan.is_some() && path.is_some());
        Ok::<_, PreviewArmingError>(())
    })?;
    drop(job.try_take_completion().ok_or("Actual preview terminal missing")?);
    drop(job);
    drop(native);
    owner.settle()
}

struct ForeignRelease;
impl MemoryRelease for ForeignRelease {
    fn release(&self, _: u64) {}
}
struct ForeignPorts(AtomicUsize);
impl Clock for ForeignPorts {
    fn now(&self) -> Moment {
        self.0.fetch_add(1, Ordering::AcqRel);
        Moment::from_nanos(u64::MAX)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
impl CancelControl for ForeignPorts {
    fn is_requested(&self) -> bool {
        self.0.fetch_add(1, Ordering::AcqRel);
        true
    }
    fn cancelled(&self) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[test]
fn preview_counterfeit_matching_context_and_memory_reject_before_any_caller_port() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65536)?;
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let mut job = worker(&owner, spec, move |context| {
        let ports = Arc::new(ForeignPorts(AtomicUsize::new(0)));
        let fake = JobContext::new(
            context.owner_id(),
            context.scope_id(),
            context.spec(),
            context.deadline(),
            ports.clone(),
            ports.clone(),
            AdmittedJobResources::new(MemoryLease::new(65536, Box::new(ForeignRelease)), None),
        );
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:\bounded-preview.png"));
        assert!(client.bind_preview_source(&fake, &mut loan, &mut path, identity).is_err());
        assert!(loan.is_some() && path.is_some());
        let matching = JobContext::new(
            context.owner_id(),
            context.scope_id(),
            context.spec(),
            context.deadline(),
            ports.clone(),
            ports.clone(),
            AdmittedJobResources::new(context.memory().clone(), None),
        );
        let seal = client.bind_preview_source(&matching, &mut loan, &mut path, identity)?;
        seal.check(&matching, identity)?;
        assert!(!seal.original_for_owner_fixture().cancellation_requested());
        assert_eq!(seal.original_for_owner_fixture().now(), context.now());
        assert_eq!(ports.0.load(Ordering::Acquire), 0);
        Ok::<_, PreviewArmingError>(seal)
    })?;
    drop(job.try_take_completion().ok_or("Actual preview terminal missing")?);
    drop(job);
    drop(native);
    close(owner)
}

#[test]
fn preview_original_campaign_expiry_rejects_before_inputs_or_job_flag_are_consumed() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65536)?;
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let clock = owner.clock.clone();
    let until = loan.original_deadline();
    let mut job = worker(&owner, spec, move |context| {
        clock.advance(until.as_nanos());
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:\bounded-preview.png"));
        assert!(matches!(
            client.bind_preview_source(context, &mut loan, &mut path, identity),
            Err(PreviewArmingError::Admission(AdmissionError::DeadlineExpired { .. }))
        ));
        assert!(loan.is_some() && path.is_some());
        Ok::<_, PreviewArmingError>(())
    })?;
    drop(job.try_take_completion().ok_or("Actual preview terminal missing")?);
    drop(job);
    drop(native);
    close(owner)
}

#[test]
fn preview_changed_original_control_declaration_rejects_before_worker_consumes_inputs() -> TestResult
{
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65536)?;
    let custody = owner.active.work.shared.clone();
    let declared = loan.control_storage_bytes();
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let mut job = worker(&owner, spec, move |context| {
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:ounded-preview.png"));
        lock(&custody.state).preview_control_bytes = declared - 1;
        assert!(client.bind_preview_source(context, &mut loan, &mut path, identity).is_err());
        assert!(loan.is_some() && path.is_some());
        lock(&custody.state).preview_control_bytes = declared;
        let seal = client.bind_preview_source(context, &mut loan, &mut path, identity)?;
        seal.check(context, identity)?;
        Ok::<_, PreviewArmingError>(seal)
    })?;
    drop(job.try_take_completion().ok_or("Actual preview terminal missing")?);
    drop(job);
    drop(native);
    close(owner)
}
