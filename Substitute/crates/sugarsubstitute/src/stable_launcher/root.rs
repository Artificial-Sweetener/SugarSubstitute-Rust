// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The stable entry: start the pointed generation's launch engine, or its fallback, then exit.
//!
//! This path reads only the launch pointer and the engine bytes it names. It waits for each
//! engine to take over or exit, at most `ENGINE_TAKEOVER` per engine, terminating one that stays
//! silent, and always returns. Every failure ends in the caller's visible startup report. If the
//! root job has not returned a short grace after its own deadline, the process exits without it.
use std::{
    error::Error,
    io,
    num::NonZeroUsize,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

/// Pointer read and two bounded takeovers, with room for pinning each engine.
const BUDGET: Duration = Duration::from_secs(40);
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionPort, JobClass, JobContext, JobOutcome, JobSpec, Moment,
        Operation, OwnedJob, Scope, ScopeId, ScopeSpec,
    },
    startup::launcher::LauncherBudget,
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_supervision::{application_launch::start_pointed, setup_storage};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Headless owner of the one root job; it never shows a window of its own.
pub(super) struct Root {
    installation: PathBuf,
    clock: Arc<dyn Clock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    job: Option<OwnedJob<(), io::Error>>,
    /// When the process exits even if the root job never returns, with its native wake.
    exit_at: Option<(Moment, Option<Instant>)>,
    /// The root job never returned; this process exits without joining it.
    abandoned: bool,
    /// The executor was asked to stop once the root job's outcome was consumed.
    stopping: bool,
    failure: Option<Box<dyn Error>>,
    done: bool,
}
impl Root {
    pub(super) fn new(installation: PathBuf) -> Self {
        Self {
            installation,
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            scope: None,
            job: None,
            exit_at: None,
            abandoned: false,
            stopping: false,
            failure: None,
            done: false,
        }
    }
    /// Start execution and admit `body` as the one root job, due at `BUDGET`.
    fn admit(
        &mut self,
        wake: Arc<dyn CompletionWake>,
        body: impl FnOnce(JobContext) -> io::Result<()> + Send + 'static,
    ) -> Result<()> {
        let owner = ExecutionOwner::new(self.clock.clone(), wake)?;
        let client = owner.client();
        let deadline = client.now()?.checked_add(BUDGET)?;
        let exit_at = LauncherBudget::exit_at(deadline)?;
        self.exit_at =
            Some((exit_at, Instant::now().checked_add(BUDGET + LauncherBudget::EXIT_GRACE)));
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        self.job = Some(client.submit_blocking(
            &scope,
            JobSpec {
                id: sugarsubstitute_contracts::execution::JobId::new(1)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(deadline),
                reservation_bytes: setup_storage::POINTER_RESERVATION_BYTES,
                replacement: None,
            },
            body,
        )?);
        self.owner = Some(owner);
        self.client = Some(client);
        self.scope = Some(scope);
        Ok(())
    }
    /// Exit without a root job that never returned. Its worker and native custody stay with
    /// this process, which is about to end; the kernel releases every handle it holds.
    fn abandon(&mut self) {
        tracing::error!(
            operation = "stable_entry_exit_unsettled",
            "The stable entry's work did not return in time; exiting without it"
        );
        std::mem::forget(self.job.take());
        std::mem::forget(self.scope.take());
        std::mem::forget(self.client.take());
        // Dropping the owner would join the worker that never returned.
        std::mem::forget(self.owner.take());
        self.abandoned = true;
        self.done = true;
        self.failure.get_or_insert_with(|| "The stable entry did not finish in time".into());
    }
    fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        if let Some(client) = &self.client {
            client.drain_completions(max)?;
        }
        if let Some(job) = &mut self.job
            && let Some(completion) = job.try_take_completion()
        {
            self.job = None;
            self.done = true;
            match completion.outcome {
                JobOutcome::Success(()) => {}
                JobOutcome::Failure(cause) => {
                    self.failure = Some(Box::new(cause));
                }
                other => {
                    self.failure =
                        Some(format!("Stable entry ended with {:?}", other.kind()).into());
                }
            }
        }
        if !self.abandoned && self.exit_at.is_some_and(|(at, _)| self.clock.now() >= at) {
            self.abandon();
        }
        Ok(if self.done { HeadlessStep::Shutdown } else { HeadlessStep::Active })
    }
}
impl HeadlessLifetime for Root {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        let installation = self.installation.clone();
        self.admit(wake, move |context| {
            start_pointed(&installation, &context).map_err(io::Error::other)
        })
    }
    fn prepare_native_runtime(&mut self, max: NonZeroUsize) -> Result<StartupPreparationStep> {
        Ok(match self.advance(max)? {
            HeadlessStep::Active => StartupPreparationStep::Waiting,
            HeadlessStep::Shutdown => StartupPreparationStep::Closing,
        })
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.advance(max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        if self.abandoned { None } else { self.exit_at.and_then(|(_, wake)| wake) }
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if let Some(job) = &self.job {
            job.cancel();
        }
        Ok(())
    }
    /// Settled only once the root job's outcome is consumed and the executor's workers have
    /// actually stopped; the executor is asked to stop exactly once, after that outcome.
    fn poll_shutdown(&mut self) -> Result<bool> {
        self.advance(NonZeroUsize::MIN.saturating_add(31))?;
        if self.job.is_some() {
            return Ok(false);
        }
        let Some(owner) = &mut self.owner else { return Ok(true) };
        if !self.stopping {
            let until = self.clock.now().checked_add(LauncherBudget::EXIT_GRACE)?;
            owner.request_shutdown(until);
            self.stopping = true;
        }
        Ok(owner.poll_shutdown()?.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if self.job.is_some() {
            return Err("Stable entry still owns its job".into());
        }
        if self.abandoned {
            return self.failure.take().map_or(Ok(()), Err);
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.scope = None;
        self.client = None;
        self.owner = None;
        self.failure.take().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
#[path = "root_tests.rs"]
mod tests;
