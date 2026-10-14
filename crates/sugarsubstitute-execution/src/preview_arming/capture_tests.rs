// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use the actual registered capture, central ordinary dispatch and typed terminal owner.
//! Only external authenticated kernel facts and the external counter read are faked.
use super::*;
use crate::{
    admission::RequestKind,
    preview_arming::*,
    task::{Job, JobHandle, QueuedJob},
};
use std::path::PathBuf;
use sugarsubstitute_contracts::startup::{LaunchId, MainIdentity};

fn fixture_creator(
    owner: &RegisteredOwner,
    native: &Scope,
) -> TestResult<Option<crate::desktop_capture::creator::AuthenticatedDesktopCreator>> {
    Ok(crate::desktop_capture::creator::AuthenticatedDesktopCreator::owner_fixture(
        &owner.core,
        native.id(),
        owner.active.work.shared.cleanup.full_target,
    )?)
}
fn borrow_run_source(
    owner: &RegisteredOwner,
    native: &Scope,
    run: sugarsubstitute_contracts::lifecycle::RunKey,
) -> TestResult<PreviewSourceLoan> {
    let client = crate::ExecutionClient { core: owner.core.clone() };
    Ok(client.borrow_preview_owner_fixture(
        &owner.session,
        native,
        run,
        CounterDeadline::new(2100, 10)?,
    )?)
}
fn source(owner: &RegisteredOwner) -> TestResult<(Scope, PreviewSourceLoan)> {
    let native = owner.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        Some(&owner.parent),
        true,
    )?;
    let run = owner
        .active
        .work
        .shared
        .cleanup
        .full_target
        .desktop()
        .ok_or("Actual Desktop target missing")?;
    lock(&owner.active.work.shared.state).authenticated_creator = fixture_creator(owner, &native)?;
    let loan = borrow_run_source(owner, &native, run)?;
    Ok((native, loan))
}
fn image(owner: &RegisteredOwner) -> TestResult<MainIdentity> {
    Ok(MainIdentity {
        run: owner
            .active
            .work
            .shared
            .cleanup
            .full_target
            .desktop()
            .ok_or("Actual Desktop target missing")?,
        launch: LaunchId::new(2)?,
        navigation: Generation::new(2)?,
    })
}
fn spec_for(loan: &PreviewSourceLoan, bytes: u64) -> TestResult<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(2)?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(loan.original_deadline()),
        reservation_bytes: bytes,
        replacement: None,
    })
}
/// Dispatch the same original central record and execute its owned factory once.
/// This isolates owner behavior from external worker timing without creating threads.
fn worker<T: Send + 'static, E: Send + 'static>(
    owner: &RegisteredOwner,
    spec: JobSpec,
    action: impl FnOnce(&JobContext) -> Result<T, E> + Send + 'static,
) -> TestResult<OwnedJob<T, E>> {
    let core = owner.core.clone();
    let job =
        core.admit(&owner.scope, spec, RequestKind::Blocking, |context, cancel, timing| {
            let job =
                Job::new(owner.scope.id(), spec, timing.admitted, cancel, Arc::downgrade(&core));
            let delivery = job.clone();
            let queued = QueuedJob {
                context,
                job: job.clone(),
                work: Work::Blocking(Box::new(move |context, started| {
                    let result = action(&context);
                    let outcome = match result {
                        Ok(value) => JobOutcome::Success(value),
                        Err(cause) => JobOutcome::Failure(cause),
                    };
                    delivery.finish_at(&context, Some(started), context.now(), outcome);
                    Box::pin(std::future::ready(()))
                })),
            };
            (queued, OwnedJob::new(owner.scope.id(), spec, Box::new(JobHandle(job))))
        })?;
    let queued = core
        .take(|class| class == JobClass::InfrastructureOrdinary)
        .ok_or("Actual ordinary worker not dispatched")?;
    let Work::Blocking(factory) = queued.work else {
        return Err("Actual ordinary work misrouted".into());
    };
    drop(factory(queued.context, Moment::from_nanos(0)));
    Ok(job)
}
fn close(owner: RegisteredOwner) -> TestResult {
    owner.session.control().request_cleanup();
    owner.settle()
}

#[test]
fn preview_actual_registered_source_survives_setup_expiry_and_binds_current_retry_once()
-> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65536)?;
    let until = loan.original_deadline();
    let counter = loan.counter_deadline();
    owner.clock.advance(101_000_000_000);
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let again = client.borrow_preview_owner_fixture(&owner.session, &native, identity.run, counter);
    assert!(again.is_err());
    let work = client.clone();
    let mut job = worker(&owner, spec, move |context| {
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:\bounded-preview.png"));
        let seal = work.bind_preview_source(context, &mut loan, &mut path, identity)?;
        assert!(loan.is_none() && path.is_none());
        assert_eq!(seal.original_deadline(), until);
        assert_eq!(seal.counter_deadline(), counter);
        seal.check(context, identity)?;
        let mut changed = identity;
        changed.navigation = Generation::new(3).map_err(|cause| {
            PreviewArmingError::Admission(AdmissionError::Unexpected(crate::runtime::error(cause)))
        })?;
        assert!(seal.check(context, changed).is_err());
        assert!(work.bind_preview_source(context, &mut loan, &mut path, identity).is_err());
        Ok::<_, PreviewArmingError>(seal)
    })?;
    let completion = job.try_take_completion().ok_or("Actual preview terminal missing")?;
    let JobOutcome::Success(seal) = completion.outcome else {
        return Err("Actual preview failed".into());
    };
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304 + 65536);
    assert!(seal.check(&seal_original(&seal)?, identity).is_err());
    drop(completion.memory);
    drop(job);
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304 + 65536);
    drop(seal);
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    drop(native);
    close(owner)
}
// The test observes the original checked value only through its private owner module.
fn seal_original(seal: &PreviewArmingSeal) -> TestResult<JobContext> {
    Ok(seal.original_for_owner_fixture())
}

#[test]
fn preview_preclaim_exhaustion_retains_original_loan_path_and_purpose_slot() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65536)?;
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let mut job = worker(&owner, spec, move |context| {
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:\bounded-preview.png"));
        let claim = context
            .memory()
            .claim(65536)
            .map_err(|cause| PreviewArmingError::Admission(AdmissionError::Memory(cause)))?;
        assert!(matches!(
            client.bind_preview_source(context, &mut loan, &mut path, identity),
            Err(PreviewArmingError::Admission(AdmissionError::Memory(_)))
        ));
        assert!(loan.is_some() && path.is_some());
        drop(claim);
        let seal = client.bind_preview_source(context, &mut loan, &mut path, identity)?;
        seal.check(context, identity)?;
        Ok::<_, PreviewArmingError>(seal)
    })?;
    drop(job.try_take_completion().ok_or("Actual preview terminal missing")?);
    drop(job);
    drop(native);
    close(owner)
}

#[test]
fn preview_actual_clock_cancel_vetoes_bind_without_consuming_original_inputs() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65536)?;
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let core = owner.core.clone();
    let scope = owner.scope.id();
    let clock = owner.clock.clone();
    let mut job = worker(&owner, spec, move |context| {
        clock.on_next_current_thread_observation(Box::new(move || core.cancel_scope(scope)));
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:\bounded-preview.png"));
        assert!(client.bind_preview_source(context, &mut loan, &mut path, identity).is_err());
        assert!(loan.is_some() && path.is_some());
        Ok::<_, PreviewArmingError>(())
    })?;
    drop(job.try_take_completion().ok_or("Actual preview terminal missing")?);
    drop(job);
    drop(native);
    owner.settle()
}

#[test]
fn preview_wrong_worker_budget_rejects_before_consumption() -> TestResult {
    let mut owner = RegisteredOwner::new(None)?;
    owner.register()?;
    let (native, loan) = source(&owner)?;
    let identity = image(&owner)?;
    let spec = spec_for(&loan, 65535)?;
    let client = crate::ExecutionClient { core: owner.core.clone() };
    let mut job = worker(&owner, spec, move |context| {
        let mut loan = Some(loan);
        let mut path = Some(PathBuf::from(r"D:\bounded-preview.png"));
        assert!(client.bind_preview_source(context, &mut loan, &mut path, identity).is_err());
        assert!(loan.is_some() && path.is_some());
        Ok::<_, PreviewArmingError>(())
    })?;
    drop(job.try_take_completion().ok_or("Actual preview terminal missing")?);
    drop(job);
    drop(native);
    close(owner)
}

#[path = "capture_lifetime_tests.rs"]
mod lifetime;

#[test]
fn preview_original_control_declaration_is_bounded_once_before_source_admission() -> TestResult {
    let (internal, total) = crate::desktop_capture_control_storage_bytes();
    eprintln!(
        "PREVIEW_CONTROL_LAYOUT internal={internal} total={total} remaining={} loan={}",
        total - internal,
        PreviewSourceLoan::storage_bytes()
    );
    let mut inherited = input()?;
    for bytes in [0, PreviewSourceLoan::storage_bytes() - 1, total - internal + 1, usize::MAX] {
        assert!(inherited.reserve_preview_control_storage(bytes).is_err());
        assert_eq!(inherited.preview_control_storage(), 0);
    }
    inherited.reserve_preview_control_storage(total - internal)?;
    assert_eq!(inherited.preview_control_storage(), total - internal);
    assert!(inherited.reserve_preview_control_storage(total - internal).is_err());
    assert_eq!(inherited.preview_control_storage(), total - internal);
    Ok(())
}
#[test]
fn preview_absent_original_declaration_never_grants_early_loan() -> TestResult {
    let mut owner = RegisteredOwner::with_input(None, input()?)?;
    owner.register()?;
    let native = owner.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        Some(&owner.parent),
        true,
    )?;
    let run = owner
        .active
        .work
        .shared
        .cleanup
        .full_target
        .desktop()
        .ok_or("Actual Desktop target missing")?;
    lock(&owner.active.work.shared.state).authenticated_creator = fixture_creator(&owner, &native)?;
    assert_eq!(lock(&owner.active.work.shared.state).preview_control_bytes, 0);
    let client = crate::ExecutionClient { core: owner.core.clone() };
    assert!(
        client
            .borrow_preview_owner_fixture(
                &owner.session,
                &native,
                run,
                CounterDeadline::new(2100, 10)?
            )
            .is_err()
    );
    assert!(!lock(&owner.active.work.shared.state).preview_loan_issued);
    assert_eq!(owner.scope.snapshot().reserved_bytes, 4_194_304);
    drop(native);
    close(owner)
}

#[path = "backend_target_tests.rs"]
mod backend_target;
