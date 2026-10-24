// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain each launcher job through terminal consumption and execution shutdown.
mod cached;
mod contention;
mod delegation;
mod engine;
mod installation;
mod jobs;
mod publication;
mod selection;
mod shutdown;
#[cfg(test)]
mod startup_tests;
#[cfg(test)]
mod tests;
use std::{error::Error, io, num::NonZeroUsize, path::PathBuf, sync::Arc, time::Instant};
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{
        Clock, CompletionWake, ExecutionPort, JobClass, JobContext, JobOutcome, JobSpec, Moment,
        Operation, OwnedJob, Scope, ScopeId, ScopeSpec,
    },
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_supervision::{
    application_launch::{ActivationLaunch, ActivationOutcome, ApplicationLaunch, CreationOutcome},
    setup_storage::{self, ReservedLaunch},
    update_preparation::Worker,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
enum Work {
    Installation(Option<ApplicationLaunch>),
    Cache(Worker),
    Prepared(ApplicationLaunch),
    TrialPrepared,
    EngineOpened,
    EngineVerified,
    EnginePrepared,
    EngineStarted,
    EngineObserved(ActivationOutcome),
    Offered,
    OfferObserved(Option<CreationOutcome>),
    Launched,
    Recovered(CreationOutcome),
    Retired,
    Retiring,
}

/// The exact admitted operation controls memory, cleanup scope and optional-update fallback.
#[derive(Clone, Copy)]
enum Task {
    Installation,
    Cache,
    Select,
    Trial,
    OpenEngine,
    VerifyEngine,
    PrepareEngine,
    CreateEngine,
    ObserveEngine,
    ObserveOffer,
    StopEngine,
    Create { optional: bool },
    Recover,
    Retire,
}
struct Pending {
    task: Task,
    job: OwnedJob<Work, io::Error>,
}

/// One next action, changed only after an observed outcome or rejected admission.
enum Choice {
    Installation,
    OpenEngine,
    VerifyEngine,
    PrepareEngine,
    CreateEngine,
    ObserveEngine,
    ObserveOffer,
    StopEngine,
    Cache,
    Accepted,
    Trial,
    Create { optional: bool },
    Recover,
    Complete,
}

pub(super) struct Launcher {
    root: PathBuf,
    clock: Arc<dyn Clock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    work_scope: Option<Scope>,
    cleanup_scope: Option<Scope>,
    jobs: crate::job_sequence::JobSequence,
    job: Option<Pending>,
    launch: Option<ApplicationLaunch>,
    cached: Option<cached::Cached>,
    engine: Option<Arc<engine::Guard>>,
    activation: Option<ActivationLaunch>,
    source: Option<Arc<sugarsubstitute_supervision::application_launch::broker::ActivationSender>>,
    creator_reply: Option<CreationOutcome>,
    activation_after: Option<Moment>,
    activation_wake: Option<Instant>,
    choice: Choice,
    selection_wait: Option<contention::Wait>,
    recovery_until: Option<Moment>,
    recovery_wake: Option<Instant>,
    update_failure: Option<Box<dyn Error>>,
    until: Option<Moment>,
    cleanup_until: Option<Moment>,
    wake_at: Option<Instant>,
    closing: bool,
    completed: bool,
    stopping: bool,
    cutoff_reported: bool,
    failure: Option<Box<dyn Error>>,
    finished: bool,
}
impl Launcher {
    /// Allocate inert composition state; all filesystem and native creation runs on workers.
    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            root,
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            work_scope: None,
            cleanup_scope: None,
            jobs: crate::job_sequence::JobSequence::new(),
            job: None,
            launch: None,
            cached: None,
            engine: None,
            activation: None,
            source: None,
            creator_reply: None,
            activation_after: None,
            activation_wake: None,
            choice: Choice::Installation,
            selection_wait: None,
            recovery_until: None,
            recovery_wake: None,
            update_failure: None,
            until: None,
            cleanup_until: None,
            wake_at: None,
            closing: false,
            completed: false,
            stopping: false,
            cutoff_reported: false,
            failure: None,
            finished: false,
        }
    }
    fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        if let Some(client) = &self.client {
            client.drain_completions(max)?;
        }
        if let Some(pending) = &mut self.job
            && let Some(completion) = pending.job.try_take_completion()
        {
            let task = pending.task;
            self.job = None;
            match completion.outcome {
                JobOutcome::Success(Work::Installation(launch)) => {
                    self.installed(launch);
                }
                JobOutcome::Success(Work::Cache(worker)) => {
                    self.cached = Some(cached::Cached::new(worker));
                }
                JobOutcome::Success(Work::Prepared(launch)) => {
                    self.launch = Some(launch);
                    self.choice = Choice::Create { optional: false };
                }
                JobOutcome::Success(Work::TrialPrepared) => {
                    self.choice = Choice::Create { optional: true }
                }
                JobOutcome::Success(Work::EngineOpened) => self.choice = Choice::VerifyEngine,
                JobOutcome::Success(Work::EngineVerified) => self.choice = Choice::Installation,
                JobOutcome::Success(Work::EnginePrepared) => self.choice = Choice::CreateEngine,
                JobOutcome::Success(Work::EngineStarted) => self.choice = Choice::ObserveEngine,
                JobOutcome::Success(Work::EngineObserved(outcome)) => {
                    self.activation_observed(outcome)?
                }
                JobOutcome::Success(Work::Offered) => self.choice = Choice::ObserveOffer,
                JobOutcome::Success(Work::OfferObserved(outcome)) => {
                    self.offer_observed(outcome)?
                }
                JobOutcome::Success(Work::Launched) => {
                    self.completed = true;
                    self.choice = Choice::Complete;
                }
                JobOutcome::Success(Work::Recovered(outcome)) => self.recovered(outcome)?,
                JobOutcome::Success(Work::Retired) => {
                    self.launch = None;
                    self.engine = None;
                    self.source = None;
                }
                JobOutcome::Success(Work::Retiring) => self.pace_activation()?,
                JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(cause) => self.failed(task, cause.into())?,
                other => self.failed(
                    task,
                    format!("Launcher operation ended with {:?}", other.kind()).into(),
                )?,
            }
        }
        self.advance_cached()?;
        if !self.closing && self.recovery_until.is_some_and(|until| self.clock.now() >= until) {
            return Err("Startup trial recovery reached its original cutoff".into());
        }
        if !self.closing
            && !self.completed
            && self.until.is_some_and(|until| self.clock.now() >= until)
        {
            return Err(self.selection_wait.take().map_or_else(
                || "Launcher startup reached its original cutoff".into(),
                |wait| wait.cause,
            ));
        }
        if !self.closing && !self.completed && self.job.is_none() {
            self.select_next()?;
        }
        Ok(if self.closing || self.completed {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
}
impl HeadlessLifetime for Launcher {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Launcher cannot start twice or after close".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Launcher execution absent")?.client();
        self.until = Some(EntrySetupBudget::new(client.now()?)?.until());
        self.wake_at = Instant::now().checked_add(EntrySetupBudget::DURATION);
        self.work_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.cleanup_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?);
        self.client = Some(client);
        self.select_next()
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
        if self.cutoff_reported {
            return None;
        }
        [
            self.wake_at,
            self.recovery_wake,
            self.activation_wake,
            self.selection_wait.as_ref().map(|wait| wait.wake_at),
            self.cached.as_ref().and_then(cached::Cached::next_deadline),
        ]
        .into_iter()
        .flatten()
        .min()
    }
    fn request_shutdown(&mut self) -> Result<()> {
        self.begin_close()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        self.poll_close()
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if self.finished || !self.closing || !self.poll_shutdown()? {
            return Err("Launcher still owns active work".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        if self.job.is_some()
            || self.launch.is_some()
            || self.cached.is_some()
            || self.engine.is_some()
            || self.activation.is_some()
            || self.source.is_some()
            || self.cutoff_reported
        {
            return Err("Launcher custody did not retire before its original cutoff".into());
        }
        if let Some(cause) = self.failure.take() {
            return Err(cause);
        }
        self.work_scope = None;
        self.cleanup_scope = None;
        self.client = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}
