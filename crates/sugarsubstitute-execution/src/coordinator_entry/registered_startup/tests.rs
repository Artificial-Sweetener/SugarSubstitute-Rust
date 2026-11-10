// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate, archive and reissue through real Core workers and original custody.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, execution::*, lifecycle::*};
const BYTES: u64 = 4 * 1024 * 1024;
fn request_snapshot(
    rig: &Rig,
    boundary: &Arc<Boundary>,
    session: &CoordinatorEntryServerSession,
) -> TestResult<PreparedStartupSnapshot> {
    activate(rig, session)?;
    boundary.connect()?;
    let EntryTransportObservation::RequestAuthenticated { request, .. } =
        fact(rig, &session.control, |fact| {
            matches!(fact, EntryTransportObservation::RequestAuthenticated { .. })
        })?
    else {
        return Err("Actual authenticated startup missing".into());
    };
    request.into_startup_snapshot().map_err(|failure| failure.error.into())
}
fn projection_spec(id: u64, until: Moment) -> TestResult<JobSpec> {
    let mut spec = spec(id, JobClass::InfrastructureLifecycle, BYTES)?;
    spec.operation = Operation::NativeLifecycle;
    spec.deadline = Some(until);
    Ok(spec)
}
fn registered(
    rig: &Rig,
    startup: PreparedStartupSnapshot,
) -> TestResult<(RegisteredStartupSource, MemoryLease)> {
    registered_profile(rig, startup, request()?.profile_directory().to_path_buf())
}
fn registered_profile(
    rig: &Rig,
    mut startup: PreparedStartupSnapshot,
    profile: std::path::PathBuf,
) -> TestResult<(RegisteredStartupSource, MemoryLease)> {
    let until = startup.launch.as_ref().ok_or("Original Entry missing")?.until();
    let original_bytes = startup.original_body()?.to_vec();
    let client = rig.client.clone();
    let original = run()?;
    let mut job =
        rig.client.submit_blocking(&rig.scope, projection_spec(2, until)?, move |context| {
            let source =
                client.register_startup_source(&mut startup, &context, original, &profile)?;
            assert!(matches!(
                client.register_startup_source(&mut startup, &context, original, &profile),
                Err(AdmissionError::InvalidCaptureAdmission)
            ));
            assert_eq!(
                startup
                    .original_body()
                    .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?,
                original_bytes
            );
            assert_eq!(
                source
                    .state()
                    .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?
                    .body
                    .as_ref(),
                original_bytes
            );
            assert_eq!(
                startup
                    .body()
                    .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?,
                source
                    .state()
                    .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?
                    .child_body
                    .as_ref()
            );
            assert!(source.view().is_ok());
            drop(startup);
            Ok::<_, AdmissionError>(source)
        })?;
    let completed = rig.completion(&mut job)?;
    let JobOutcome::Success(source) = completed.outcome else {
        return Err("Actual archive producer failed".into());
    };
    Ok((source, completed.memory))
}
fn next(previous: RunKey) -> TestResult<RunKey> {
    Ok(RunKey { generation: previous.generation.next()?, ..previous })
}
#[test]
fn registered_startup_releases_original_loan_and_preserves_exact_source_credit_until_last_alias()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let startup = request_snapshot(&rig, &boundary, &session)?;
    let expected = startup.body()?.to_vec();
    let (source, memory) = registered(&rig, startup)?;
    let alias = source.clone();
    assert!(source.same_source(&alias));
    assert_eq!(source.state()?.body.as_ref(), expected);
    assert_eq!(source.retained_bytes(), REGISTERED_STARTUP_BYTES);
    assert!(memory.claim(BYTES - REGISTERED_STARTUP_BYTES + 1).is_err());
    drop(memory);
    assert_eq!(rig.scope.snapshot().reserved_bytes, BYTES);
    drop(source);
    assert_eq!(rig.scope.snapshot().reserved_bytes, BYTES);
    drop(alias);
    let core = rig.client.core.clone();
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    drop(session);
    drop(boundary);
    drop(rig);
    assert_eq!(lock(&core.state).bytes, 0);
    Ok(())
}
#[test]
fn registered_startup_new_capsule_uses_exact_body_after_old_entry_deadline_without_reauthentication()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let startup = request_snapshot(&rig, &boundary, &session)?;
    let expected = startup.body()?.to_vec();
    let old_until = startup.launch.as_ref().ok_or("Original Entry missing")?.until();
    let (source, memory) = registered(&rig, startup)?;
    drop(memory);
    rig.clock.advance(old_until.as_nanos() + 1);
    let previous = run()?;
    let new = next(previous)?;
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let client = rig.client.clone();
    let retained = source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        projection_spec(3, original.until())?,
        move |context| {
            let capsule =
                client.restart_startup_snapshot(&retained, previous, new, original, &context)?;
            assert!(capsule.launch.is_none());
            assert_eq!(
                capsule
                    .body()
                    .map_err(|error| AdmissionError::Unexpected(crate::runtime::error(error)))?,
                expected
            );
            assert!(matches!(
                client.restart_startup_snapshot(&retained, previous, new, original, &context),
                Err(AdmissionError::InvalidCaptureAdmission)
            ));
            Ok::<_, AdmissionError>(capsule)
        },
    )?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(capsule) = completion.outcome else {
        return Err("Actual restart producer failed".into());
    };
    let binding = capsule.admission_binding()?;
    assert_eq!(
        binding.validate_current_locked(
            &rig.client.core,
            &lock(&rig.client.core.state),
            new,
            rig.client.now()?
        )?,
        original.until()
    );
    rig.clock.advance(original.until().as_nanos() - rig.client.now()?.as_nanos());
    assert!(
        binding
            .validate_current_locked(
                &rig.client.core,
                &lock(&rig.client.core.state),
                new,
                rig.client.now()?
            )
            .is_err()
    );
    drop(binding);
    drop(capsule);
    drop(source);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
#[test]
fn registered_startup_foreign_owner_wrong_generation_and_terminal_context_cannot_issue()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let previous = run()?;
    let new = next(previous)?;
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let client = rig.client.clone();
    let retained = source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        projection_spec(3, original.until())?,
        move |context| {
            let mut foreign = new;
            foreign.owner.epoch = CoordinatorEpoch::new([9; 16])
                .map_err(|_| AdmissionError::InvalidCaptureAdmission)?;
            assert!(
                client
                    .restart_startup_snapshot(&retained, previous, foreign, original, &context)
                    .is_err()
            );
            assert!(
                client
                    .restart_startup_snapshot(&retained, previous, previous, original, &context)
                    .is_err()
            );
            Ok::<_, AdmissionError>(context)
        },
    )?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(context) = completion.outcome else {
        return Err("Actual negative producer failed".into());
    };
    assert!(
        rig.client.restart_startup_snapshot(&source, previous, new, original, &context).is_err()
    );
    assert_eq!(
        source.state()?.progress.lock().map_err(|_| "Original progress poisoned")?.current,
        previous
    );
    let foreign = Rig::new()?;
    assert!(
        foreign
            .client
            .restart_startup_snapshot(&source, previous, new, original, &context)
            .is_err()
    );
    drop(context);
    drop(completion.memory);
    drop(source);
    session.control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Success(_)));
    Ok(())
}
#[test]
fn registered_startup_original_owner_cancel_vetoes_final_capsule() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (entry_scope, mut session) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &session)?)?;
    drop(memory);
    let previous = run()?;
    let new = next(previous)?;
    let original = EntrySetupBudget::new(rig.client.now()?)?;
    let client = rig.client.clone();
    let retained = source.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        projection_spec(3, original.until())?,
        move |context| {
            client.restart_startup_snapshot(&retained, previous, new, original, &context)
        },
    )?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(capsule) = completion.outcome else {
        return Err("Actual restart producer failed".into());
    };
    let binding = capsule.admission_binding()?;
    rig.client.core.cancel_scope(entry_scope.id());
    assert!(
        binding
            .validate_current_locked(
                &rig.client.core,
                &lock(&rig.client.core.state),
                new,
                rig.client.now()?
            )
            .is_err()
    );
    drop(binding);
    drop(capsule);
    drop(source);
    session.control.close();
    drop(rig.completion(&mut session.completion)?);
    Ok(())
}

#[path = "admission_tests.rs"]
mod admission;

#[path = "quiescence_tests.rs"]
mod quiescence;

#[path = "witness_tests.rs"]
mod witness;

#[path = "recovery_tests.rs"]
mod recovery;

#[path = "profile_tests.rs"]
mod profile;
