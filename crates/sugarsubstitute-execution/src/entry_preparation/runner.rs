// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual preparation outcomes on the existing finite blocking facility.

use super::{
    EntryPreparationContext, EntryPreparationInputError, EntryPreparationSuccess,
    core_binding::OriginalPreparation,
    outcome::{EntryPreparationCause, EntryPreparationFailure},
    owner::EntryPreparationOwner,
    pending::Pending,
};
use crate::{
    admission::Core,
    task::{Job, panic_cause},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{JobContext, JobOutcome, Moment},
};

/// Use the same admitted Blocking work, never generic stop-driven result replacement.
pub(crate) async fn run<T, E, P>(
    core: Arc<Core>,
    job: Arc<Job<EntryPreparationSuccess<T>, EntryPreparationFailure<E>>>,
    context: JobContext,
    setup: EntrySetupBudget,
    dispatched: Moment,
    factory: P,
) where
    T: Send + 'static,
    E: Send + 'static,
    P: FnOnce(EntryPreparationContext) -> Result<T, E> + Send + 'static,
{
    let worker_context = context.clone();
    let worker = tokio::task::spawn_blocking(move || {
        prepare(core, worker_context, setup, dispatched, factory)
    });
    match worker.await {
        Ok((started, ended, outcome)) => job.finish_at(&context, Some(started), ended, outcome),
        Err(error) => job.finish_at(
            &context,
            Some(dispatched),
            dispatched,
            JobOutcome::Panicked(sugarsubstitute_contracts::execution::PanicCause::new(
                error.to_string(),
            )),
        ),
    }
}

type Prepared<T, E> =
    (Moment, Moment, JobOutcome<EntryPreparationSuccess<T>, EntryPreparationFailure<E>>);

fn prepare<T, E, P>(
    core: Arc<Core>,
    context: JobContext,
    setup: EntrySetupBudget,
    dispatched: Moment,
    factory: P,
) -> Prepared<T, E>
where
    T: Send + 'static,
    E: Send + 'static,
    P: FnOnce(EntryPreparationContext) -> Result<T, E>,
{
    let (binding, memory) = match OriginalPreparation::mint(&core, &context, setup) {
        Ok(parts) => parts,
        Err(cause) => {
            let mut failure =
                EntryPreparationFailure::new(EntryPreparationCause::Preparation(cause), None);
            dispose_factory(factory, &mut failure);
            return (dispatched, dispatched, JobOutcome::Failure(failure));
        }
    };
    let mut owner = match EntryPreparationOwner::new(core.clone(), binding, memory) {
        Ok(owner) => owner,
        Err((cause, memory)) => {
            let mut failure = EntryPreparationFailure::unallocated(cause, memory);
            dispose_factory(factory, &mut failure);
            return (dispatched, dispatched, JobOutcome::Failure(failure));
        }
    };
    // This outer alias survives every provider, factory and native callback catch.
    let callback = owner.callback();
    let mut pending = match callback {
        Err(cause) => {
            let mut failure = EntryPreparationFailure::new(
                EntryPreparationCause::Preparation(cause),
                Some(owner.clone()),
            );
            dispose_factory(factory, &mut failure);
            Pending::failure(failure)
        }
        Ok(callback) => {
            let registered =
                owner.checked(|| owner.register()).and_then(|()| owner.check_publication());
            let pending = match registered {
                Err(cause) => {
                    let mut failure = EntryPreparationFailure::new(
                        EntryPreparationCause::Preparation(cause),
                        Some(owner.clone()),
                    );
                    dispose_factory(factory, &mut failure);
                    Pending::failure(failure)
                }
                Ok(_) => match catch_unwind(AssertUnwindSafe(|| {
                    factory(EntryPreparationContext::new(owner.clone()))
                })) {
                    Ok(Ok(value)) => Pending::value(value),
                    Ok(Err(cause)) => Pending::failure(EntryPreparationFailure::new(
                        EntryPreparationCause::Factory(cause),
                        Some(owner.clone()),
                    )),
                    Err(payload) => {
                        let cause = panic_cause(payload);
                        owner.fail_owner(&cause);
                        Pending::failure(EntryPreparationFailure::new(
                            EntryPreparationCause::Panicked(cause),
                            Some(owner.clone()),
                        ))
                    }
                },
            };
            drop(callback);
            pending
        }
    };
    if pending.has_value()
        && owner.payload().is_ok_and(|payload| payload.native_failure.get().is_some())
    {
        pending = pending.adopt_failure(EntryPreparationFailure::new(
            EntryPreparationCause::NativeEnvironment,
            Some(owner.clone()),
        ));
    }
    if !pending.has_value() {
        owner.close_publication();
    }
    // The original returned outcome is owned before this lazy observation.
    if let Err(cause) = owner.observe_control() {
        pending = pending.observe(cause, &owner);
    }
    owner.close_acquisition();
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| owner.progress_retirement())) {
        let cause = panic_cause(payload);
        owner.fail_owner(&cause);
        pending = pending.observe(
            EntryPreparationInputError::Control(
                super::EntryPreparationControlCause::ProviderPanic(cause),
            ),
            &owner,
        );
    }
    if pending.has_value()
        && let Err(cause) = owner.check_publication()
    {
        pending = pending.observe(cause, &owner);
    }
    let ended = owner
        .original_binding()
        .map_or(dispatched, |binding| binding.last_observed().max(dispatched));
    if !owner.retirement_settled()
        && let Err((cause, returned)) = core.handoff_entry_preparation(owner.clone())
    {
        // Rejection does not replace the original failure or release held data.
        // The complete returned guard survives outside catch. Its final
        // unsettled payload remains held if genuine handoff could not occur.
        pending = pending.observe(cause, &returned);
        drop(returned);
    }
    let outcome = pending.finish(owner);
    (dispatched, ended, outcome)
}

fn dispose_factory<P, E>(factory: P, failure: &mut EntryPreparationFailure<E>) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(factory))) {
        failure.append(EntryPreparationInputError::Control(
            super::EntryPreparationControlCause::ProviderPanic(panic_cause(payload)),
        ));
    }
}
