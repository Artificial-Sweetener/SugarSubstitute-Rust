// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise genuine preparation admission, delivered controls and whole result custody.

use super::{
    EntryPreparationContext, EntryPreparationInputError, EntryPreparationSuccess,
    core_binding::PREPARATION_BYTES,
};
use crate::{
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, execution::*};

fn request(id: u64) -> TestResult<(JobSpec, EntrySetupBudget)> {
    let setup = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut request = spec(id, JobClass::InfrastructureLifecycle, PREPARATION_BYTES)?;
    request.operation = Operation::NativeLifecycle;
    request.deadline = Some(setup.until());
    Ok((request, setup))
}

fn delivered_context(
    rig: &Rig,
    scope: &Scope,
    id: u64,
) -> TestResult<EntryPreparationSuccess<EntryPreparationContext>> {
    let (request, setup) = request(id)?;
    let mut job = rig
        .client
        .submit_entry_preparation(scope, request, setup, Ok::<_, io::Error>)
        .map_err(|rejected| rejected.error)?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(success) = completion.outcome else {
        return Err("Genuine preparation did not return its sealed context".into());
    };
    drop(completion.memory);
    Ok(success)
}

#[test]
fn acknowledged_preparation_observes_actual_closed_ancestor_without_synthetic_cancellation()
-> TestResult {
    let rig = Rig::new()?;
    let child = rig
        .client
        .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, Some(&rig.scope))?;
    let success = delivered_context(&rig, &child, 1)?;
    let context = success.value();
    assert!(!lock(&rig.client.core.state).jobs.contains_key(&context.job_context().job_id()));
    assert_eq!(context.check()?, Moment::from_nanos(0));
    rig.client.core.close_scope(rig.scope.id());
    assert!(!context.job_context().cancellation_requested());
    assert!(!child.snapshot().closed);
    let cause = context.check().err().ok_or("Closed ancestor permitted fresh publication")?;
    assert!(matches!(cause.cause(), Some(EntryPreparationInputError::Closed)));
    assert_eq!(child.snapshot().reserved_bytes, PREPARATION_BYTES);
    Ok(())
}

#[test]
fn actual_original_provider_can_cancel_same_scope_after_acknowledgement() -> TestResult {
    let rig = Rig::new()?;
    let success = delivered_context(&rig, &rig.scope, 1)?;
    let core = rig.client.core.clone();
    let id = rig.scope.id();
    let returned = Arc::new(AtomicBool::new(false));
    let observed = returned.clone();
    rig.clock.on_next_current_thread_observation(Box::new(move || {
        core.cancel_scope(id);
        observed.store(true, Ordering::Release);
    }));
    let cause = success.value().check().err().ok_or("Actual cancellation was ignored")?;
    assert!(returned.load(Ordering::Acquire));
    assert!(matches!(
        cause.cause(),
        Some(EntryPreparationInputError::Control(super::EntryPreparationControlCause::Cancelled,))
    ));
    assert!(success.value().job_context().cancellation_requested());
    assert_eq!(rig.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    Ok(())
}

#[test]
fn escaped_provider_failure_retains_original_claim_and_closes_fresh_publication() -> TestResult {
    let rig = Rig::new()?;
    let success = delivered_context(&rig, &rig.scope, 1)?;
    rig.clock.on_next_current_thread_observation(Box::new(|| {
        std::panic::resume_unwind(Box::new("actual original preparation clock failure"));
    }));
    let failure = success.value().check().err().ok_or("Provider failure was discarded")?;
    assert!(matches!(
        failure.cause(),
        Some(EntryPreparationInputError::Control(super::EntryPreparationControlCause::Unexpected(
            _
        ),))
    ));
    let resampled = Arc::new(AtomicBool::new(false));
    let flag = resampled.clone();
    rig.clock.on_next_current_thread_observation(Box::new(move || {
        flag.store(true, Ordering::Release);
    }));
    let closed = success.value().check().err().ok_or("Fatal publication reopened")?;
    assert!(matches!(closed.cause(), Some(EntryPreparationInputError::Closed)));
    assert!(!resampled.load(Ordering::Acquire));
    drop(closed);
    drop(success);
    assert_eq!(rig.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    let retained = failure.clone();
    drop(failure);
    assert_eq!(rig.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    drop(retained);
    let Rig { owner, client, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(client);
    Ok(())
}

struct DropProbe {
    context: JobContext,
    charged_during_drop: Arc<AtomicBool>,
}
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.charged_during_drop.store(self.context.memory().claim(1).is_err(), Ordering::Release);
    }
}

#[test]
fn generic_success_destroys_actual_data_before_releasing_whole_claim() -> TestResult {
    let rig = Rig::new()?;
    let charged_during_drop = Arc::new(AtomicBool::new(false));
    let observed = charged_during_drop.clone();
    let (request, setup) = request(1)?;
    let mut job = rig
        .client
        .submit_entry_preparation(&rig.scope, request, setup, move |context| {
            Ok::<_, io::Error>(DropProbe {
                context: context.job_context().clone(),
                charged_during_drop: observed,
            })
        })
        .map_err(|rejected| rejected.error)?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(success) = completion.outcome else {
        return Err("Actual sealed result was absent".into());
    };
    drop(completion.memory);
    drop(job);
    assert_eq!(rig.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    drop(success);
    assert!(charged_during_drop.load(Ordering::Acquire));
    let Rig { owner, client, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(client);
    Ok(())
}
