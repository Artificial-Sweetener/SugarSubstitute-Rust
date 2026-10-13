// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Submit one original preparation to the existing finite-blocking facility.

use super::{EntryPreparationContext, EntryPreparationFailure, EntryPreparationSuccess};
use crate::{
    ExecutionClient,
    admission::RequestKind,
    task::{Job, JobHandle, QueuedJob, Work},
};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{AdmissionError, JobSpec, OwnedJob, Scope},
};

/// Return the exact unexecuted factory on rejection without allocating a second custody box.
pub struct EntryPreparationAdmissionRejection<P> {
    /// Original central admission failure.
    pub error: AdmissionError,
    /// Identical factory and all its captured caller-owned inputs.
    pub factory: P,
}
impl ExecutionClient {
    /// Prepare on Windows using the original four-MiB job, scope and thirty-second ancestor.
    /// Successful data remains sealed with its original whole owner until actual teardown.
    /// The private typed runner preserves first failure and adopts native retirement
    /// before publishing a terminal result. It shares the existing blocking workers.
    /// # Errors
    /// Returns the identical factory without running it when actual admission rejects.
    pub fn submit_entry_preparation<T, E, P>(
        &self,
        scope: &Scope,
        spec: JobSpec,
        setup: EntrySetupBudget,
        factory: P,
    ) -> Result<
        OwnedJob<EntryPreparationSuccess<T>, EntryPreparationFailure<E>>,
        EntryPreparationAdmissionRejection<P>,
    >
    where
        T: Send + 'static,
        E: Send + 'static,
        P: FnOnce(EntryPreparationContext) -> Result<T, E> + Send + 'static,
    {
        self.core
            .admit_owned(
                scope,
                spec,
                RequestKind::EntryPreparation(setup),
                factory,
                |factory, context, cancel, admitted| {
                    let job =
                        Job::new(scope.id(), spec, admitted, cancel, Arc::downgrade(&self.core));
                    let runner_job = job.clone();
                    let core = self.core.clone();
                    let work = Work::Blocking(Box::new(move |context, dispatched| {
                        Box::pin(super::runner::run(
                            core, runner_job, context, setup, dispatched, factory,
                        ))
                    }));
                    Ok((
                        QueuedJob { context, job: job.clone(), work },
                        OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))),
                    ))
                },
            )
            .map_err(|(error, factory)| EntryPreparationAdmissionRejection { error, factory })
    }
}
