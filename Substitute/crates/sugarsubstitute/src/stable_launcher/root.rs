// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The stable entry: start the pointed generation's launch engine, or its fallback, then exit.
//!
//! This path reads only the launch pointer and the engine bytes it names. It waits for each
//! engine to take over or exit, at most `ENGINE_TAKEOVER` per engine, terminating one that stays
//! silent, and always returns. Every failure ends in the caller's visible startup report.
use std::{error::Error, io, num::NonZeroUsize, path::PathBuf, sync::Arc, time::Duration};

/// Pointer read and two bounded takeovers, with room for pinning each engine.
const BUDGET: Duration = Duration::from_secs(40);
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionPort, JobClass, JobOutcome, JobSpec, Operation, OwnedJob,
    Scope, ScopeId, ScopeSpec,
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
            failure: None,
            done: false,
        }
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
        Ok(if self.done { HeadlessStep::Shutdown } else { HeadlessStep::Active })
    }
}
impl HeadlessLifetime for Root {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        let owner = ExecutionOwner::new(self.clock.clone(), wake)?;
        let client = owner.client();
        let deadline = client.now()?.checked_add(BUDGET)?;
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let installation = self.installation.clone();
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
            move |context| start_pointed(&installation, &context).map_err(io::Error::other),
        )?);
        self.owner = Some(owner);
        self.client = Some(client);
        self.scope = Some(scope);
        Ok(())
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
    fn next_deadline(&self) -> Option<std::time::Instant> {
        None
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if let Some(job) = &self.job {
            job.cancel();
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        self.advance(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(self.job.is_none())
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if self.job.is_some() {
            return Err("Stable entry still owns its job".into());
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
