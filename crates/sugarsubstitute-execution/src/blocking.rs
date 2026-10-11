// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Supervise finite blocking work without claiming cancellation before it returns.

use crate::task::{Job, panic_cause, stop_reason, stopped_outcome};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use sugarsubstitute_contracts::execution::{JobContext, JobOutcome, Moment};

pub(crate) async fn run<T, E, M>(
    job: Arc<Job<T, E>>,
    context: JobContext,
    dispatched: Moment,
    factory: M,
) where
    T: Send + 'static,
    E: Send + 'static,
    M: FnOnce(JobContext) -> Result<T, E> + Send + 'static,
{
    let worker_context = context.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let started = worker_context.now();
        let outcome = if let Some(reason) = stop_reason(&worker_context) {
            match catch_unwind(AssertUnwindSafe(|| drop(factory))) {
                Ok(()) => stopped_outcome(reason),
                Err(payload) => JobOutcome::Panicked(panic_cause(payload)),
            }
        } else {
            match catch_unwind(AssertUnwindSafe(|| factory(worker_context))) {
                Ok(Ok(value)) => JobOutcome::Success(value),
                Ok(Err(error)) => JobOutcome::Failure(error),
                Err(payload) => JobOutcome::Panicked(panic_cause(payload)),
            }
        };
        (started, outcome)
    });
    let (started, outcome) = match handle.await {
        Err(error) => (
            dispatched,
            JobOutcome::Panicked(sugarsubstitute_contracts::execution::PanicCause::new(
                error.to_string(),
            )),
        ),
        Ok((started, outcome)) => {
            let outcome = if !matches!(&outcome, JobOutcome::Panicked(_)) {
                match stop_reason(&context) {
                    Some(reason) => match catch_unwind(AssertUnwindSafe(|| drop(outcome))) {
                        Ok(()) => stopped_outcome(reason),
                        Err(payload) => JobOutcome::Panicked(panic_cause(payload)),
                    },
                    None => outcome,
                }
            } else {
                outcome
            };
            (started, outcome)
        }
    };
    job.finish(&context, Some(started), outcome);
}
