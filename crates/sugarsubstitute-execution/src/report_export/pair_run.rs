// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe one actual blocking worker and both independently owned logical outcomes.
use crate::{
    admission::Core,
    scope::lock,
    task::{Job, Stop, panic_cause, stopped_outcome},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, JobOutcome, JobPhase, Moment,
};

fn stop(error: AdmissionError) -> Stop {
    if matches!(error, AdmissionError::DeadlineExpired { .. }) {
        Stop::TimedOut
    } else {
        Stop::Cancelled
    }
}

fn enter(core: &Core, source: &JobContext, receiving: &JobContext) -> Result<(), AdmissionError> {
    let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
    let mut state = lock(&core.state);
    for context in [source, receiving] {
        let record = crate::capture_bundle::validate::association(core, &state, context)?;
        let pair = record.report_pair.ok_or(AdmissionError::InvalidReportExportAdmission)?;
        if record.phase != JobPhase::Running
            || record.retiring
            || pair.worker.is_some()
            || pair.root != source.job_id()
            || pair.partner
                != if context.job_id() == source.job_id() {
                    receiving.job_id()
                } else {
                    source.job_id()
                }
        {
            return Err(AdmissionError::InvalidReportExportAdmission);
        }
        if let Some(deadline) =
            crate::admission_liveness::scope_only_deadline(core, &state, record)?
                .filter(|deadline| *deadline <= now)
        {
            return Err(AdmissionError::DeadlineExpired { deadline, now });
        }
    }
    for id in [source.job_id(), receiving.job_id()] {
        if let Some(pair) = state.jobs.get_mut(&id).and_then(|record| record.report_pair.as_mut()) {
            pair.worker = Some(std::thread::current().id());
        }
    }
    Ok(())
}

pub(crate) async fn run<E, M>(
    core: Arc<Core>,
    source_job: Arc<Job<bool, E>>,
    source: JobContext,
    receiving_job: Arc<Job<bool, E>>,
    receiving: JobContext,
    dispatched: Moment,
    factory: M,
) where
    E: Copy + Send + 'static,
    M: FnOnce(JobContext, JobContext) -> Result<bool, E> + Send + 'static,
{
    let worker_source = source.clone();
    let worker_receiving = receiving.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let started = worker_source.now();
        let outcome = match catch_unwind(AssertUnwindSafe(|| {
            match enter(&core, &worker_source, &worker_receiving) {
                Err(error) => {
                    drop(factory);
                    stopped_outcome(stop(error))
                }
                Ok(()) => {
                    let result = factory(worker_source.clone(), worker_receiving.clone());
                    let decision =
                        core.clock.metadata().map_err(AdmissionError::Unexpected).and_then(|now| {
                            let state = lock(&core.state);
                            crate::capture_bundle::validate::live(
                                &core,
                                &state,
                                &worker_source,
                                now,
                            )?;
                            crate::capture_bundle::validate::live(
                                &core,
                                &state,
                                &worker_receiving,
                                now,
                            )
                        });
                    match decision {
                        Err(error) => stopped_outcome(stop(error)),
                        Ok(()) => match result {
                            Ok(value) => JobOutcome::Success(value),
                            Err(error) => JobOutcome::Failure(error),
                        },
                    }
                }
            }
        })) {
            Ok(outcome) => outcome,
            Err(payload) => JobOutcome::Panicked(panic_cause(payload)),
        };
        (started, outcome)
    });
    let (started, outcome) = match handle.await {
        Ok(result) => result,
        Err(error) => (
            dispatched,
            JobOutcome::Panicked(sugarsubstitute_contracts::execution::PanicCause::new(
                error.to_string(),
            )),
        ),
    };
    let receiving_outcome = match &outcome {
        JobOutcome::Success(value) => JobOutcome::Success(*value),
        JobOutcome::Failure(error) => JobOutcome::Failure(*error),
        JobOutcome::Panicked(cause) => JobOutcome::Panicked(cause.clone()),
        JobOutcome::Cancelled => JobOutcome::Cancelled,
        JobOutcome::TimedOut => JobOutcome::TimedOut,
        JobOutcome::Coalesced { replacement } => {
            JobOutcome::Coalesced { replacement: *replacement }
        }
    };
    let ended = source.now();
    source_job.finish_at(&source, Some(started), ended, outcome);
    receiving_job.finish_at(&receiving, Some(started), ended, receiving_outcome);
}
