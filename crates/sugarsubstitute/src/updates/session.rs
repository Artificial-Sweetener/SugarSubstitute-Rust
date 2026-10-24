// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One update process's executor, fixed campaign cutoff and observed resource retirement.
mod lifetime;
#[cfg(test)]
mod tests;
use crate::{authority_clock::Receipt, job_sequence::JobSequence};
use std::{
    error::Error,
    io,
    num::NonZeroUsize,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_supervision::update_preparation::{Progress, Worker};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Factory = Box<dyn FnOnce(&JobContext) -> io::Result<Option<Worker>> + Send>;
use super::CAMPAIGN;
const CLEANUP: Duration = Duration::from_secs(15);

/// Own preparation or one-shot verification; never retain application run, profile or ComfyUI custody.
/// Native process containment must enforce the outer bound if an OS call cannot retire.
pub(super) struct Session {
    factory: Option<Factory>,
    initialization_bytes: u64,
    initialization_succeeded: bool,
    clock: Arc<dyn Clock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    work: Option<Scope>,
    cleanup: Option<Scope>,
    jobs: JobSequence,
    initializing: Option<OwnedJob<Option<Worker>, io::Error>>,
    worker: Option<Worker>,
    engine: Option<super::engine::Engine>,
    receipt: Option<Receipt>,
    until: Option<Moment>,
    cleanup_until: Option<Moment>,
    wake_at: Option<Instant>,
    closing: bool,
    stopping: bool,
    finished: bool,
    initialized: bool,
    pub(super) result: Option<Progress>,
}
impl Session {
    /// Keep construction inert; distribution decoding is admitted by `start`.
    pub(super) fn new(initialization_bytes: u64, factory: Factory) -> Self {
        Self {
            factory: Some(factory),
            initialization_bytes,
            initialization_succeeded: false,
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            work: None,
            cleanup: None,
            jobs: JobSequence::new(),
            initializing: None,
            worker: None,
            engine: None,
            receipt: None,
            until: None,
            cleanup_until: None,
            wake_at: None,
            closing: false,
            stopping: false,
            finished: false,
            initialized: false,
            result: None,
        }
    }

    /// Consume terminal values before admitting another phase; never block on worker storage.
    fn advance(&mut self, maximum: NonZeroUsize) -> Result<HeadlessStep> {
        let batch = match &self.client {
            Some(client) => client.drain_completions(maximum)?,
            None => CompletionBatch { notifications: Vec::new(), remaining: 0 },
        };
        if let Some(job) = &mut self.initializing
            && let Some(done) = job.try_take_completion()
        {
            self.initializing = None;
            self.initialized = true;
            match done.outcome {
                JobOutcome::Success(worker) => {
                    self.initialization_succeeded = !self.closing;
                    self.worker = worker;
                    if self.closing
                        && let Some(worker) = &mut self.worker
                    {
                        worker.cancel();
                    }
                }
                JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(cause) => return Err(cause.into()),
                other => {
                    return Err(stopped("Update initialization", other));
                }
            }
        }
        if let Some(worker) = &mut self.worker
            && let Some(observed) = worker.poll()
        {
            if let Some(cause) = self.receipt.take().and_then(|clock| clock.take_failure()) {
                if let Some(secondary) = observed.failure {
                    tracing::error!(operation="update_clock_verification",cause=?secondary,"Verification also failed after clock failure");
                }
                return Err(cause.into());
            }
            if let Some(cause) = observed.retirement_failure {
                return Err(Box::new(io::Error::other(cause)));
            }
            if let Some(cause) = observed.failure {
                return Err(Box::new(io::Error::other(cause)));
            }
            match observed.outcome {
                JobOutcome::Success(()) => {}
                JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(cause) => return Err(Box::new(io::Error::other(cause))),
                other => {
                    return Err(stopped("Update preparation", other));
                }
            }
            if !self.closing && observed.progress == Progress::EngineReady {
                let client = self.client.as_ref().ok_or("Update execution absent")?.clone();
                let until = self.until.ok_or("Update campaign cutoff absent")?;
                let work = client.create_scope(
                    ScopeSpec { id: ScopeId::new(3)?, deadline: Some(until) },
                    Some(self.work.as_ref().ok_or("Update scope absent")?),
                )?;
                let cleanup = client.create_scope(
                    ScopeSpec { id: ScopeId::new(4)?, deadline: None },
                    Some(self.cleanup.as_ref().ok_or("Update cleanup scope absent")?),
                )?;
                self.engine = Some(super::engine::Engine::new(
                    worker.take_engine()?,
                    client,
                    work,
                    cleanup,
                    until,
                ));
            } else if !self.closing
                && matches!(
                    observed.progress,
                    Progress::Ready
                        | Progress::AlreadyPreparing
                        | Progress::Decision(_)
                        | Progress::Unavailable
                )
            {
                self.result = Some(observed.progress);
            }
        }
        if let Some(engine) = &mut self.engine {
            engine.advance(&batch, &self.jobs)?;
            if engine.succeeded() && !self.closing {
                // Child exit proves retirement, not that a particular application update is ready.
                self.result = Some(Progress::Closed);
            }
        }
        if self.closing || self.result.is_some() || (self.initialized && self.worker.is_none()) {
            return Ok(HeadlessStep::Shutdown);
        }
        if self.until.is_some_and(|until| self.clock.now() >= until) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Update preparation campaign expired",
            )
            .into());
        }
        if self.engine.is_none() && self.worker.as_ref().is_some_and(|worker| !worker.pending()) {
            self.submit(false)?;
        }
        Ok(HeadlessStep::Active)
    }

    /// A one-shot initialization role succeeds only after actual success and full execution retirement.
    /// Cancellation, early close and consumed failures cannot become a successful probe exit.
    pub(super) fn require_initialization_success(&self) -> io::Result<()> {
        if !self.finished || !self.initialization_succeeded {
            return Err(io::Error::other("Updater verification did not complete successfully"));
        }
        Ok(())
    }

    /// All phase jobs retain the original campaign deadline; cleanup gets one separate cutoff.
    fn submit(&mut self, cleanup: bool) -> Result<()> {
        let client = self.client.as_ref().ok_or("Update execution absent")?;
        let scope = if cleanup { &self.cleanup } else { &self.work }
            .as_ref()
            .ok_or("Update scope absent")?;
        let until = if cleanup { self.cleanup_until } else { self.until }
            .ok_or("Update deadline absent")?;
        self.receipt = super::admission::submit(
            self.worker.as_mut().ok_or("Update worker absent")?,
            client,
            scope,
            self.jobs.allocate()?,
            until,
        )?;
        Ok(())
    }
}

/// Preserve bounded panic diagnostics and replacement identity after actual worker termination.
pub(super) fn stopped<T, E>(stage: &'static str, outcome: JobOutcome<T, E>) -> Box<dyn Error> {
    match outcome {
        JobOutcome::Panicked(cause) => format!("{stage} panicked: {}", cause.as_str()).into(),
        JobOutcome::Coalesced { replacement } => {
            format!("{stage} was replaced by job {}", replacement.get()).into()
        }
        other => format!("{stage} ended with {:?}", other.kind()).into(),
    }
}
