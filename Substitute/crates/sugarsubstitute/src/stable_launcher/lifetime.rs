// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain each launcher job through terminal consumption and execution shutdown.
//!
//! The whole launch is bounded by [`LauncherBudget`]: optional update work is admitted only
//! before its cutoff, after which the accepted version starts, and the process exits a short
//! grace after its cleanup cutoff even if a worker never returns.
mod bound;
#[cfg(test)]
mod bound_tests;
mod cached;
mod engine;
mod installation;
mod jobs;
mod selection;
mod shutdown;
#[cfg(test)]
mod startup_tests;
#[cfg(test)]
mod tests;
use std::{error::Error, io, num::NonZeroUsize, path::PathBuf, sync::Arc, time::Instant};
use sugarsubstitute_contracts::{
    execution::{
        Clock, CompletionWake, ExecutionPort, JobClass, JobContext, JobOutcome, JobSpec, Moment,
        Operation, OwnedJob, Scope, ScopeId, ScopeSpec,
    },
    startup::launcher::LauncherBudget,
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_supervision::{
    application_launch::{ApplicationLaunch, CreationOutcome, Takeover},
    setup_storage::{self, ReservedLaunch},
    update_preparation::Worker,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
enum Work {
    Routed(setup_storage::Route),
    Installation(Option<ApplicationLaunch>),
    Cache(Worker),
    Prepared(ApplicationLaunch),
    TrialPrepared,
    EngineOpened,
    EngineVerified,
    Delegated(Takeover),
    Launched,
    Recovered(CreationOutcome),
    Retired,
}

/// The exact admitted operation controls memory, cleanup scope and optional-update fallback.
#[derive(Clone, Copy)]
enum Task {
    Route,
    Installation,
    Cache,
    Select,
    Trial,
    OpenEngine,
    VerifyEngine,
    Delegate,
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
    Route,
    Installation,
    OpenEngine,
    VerifyEngine,
    Delegate,
    Cache,
    Accepted,
    Trial,
    Create { optional: bool },
    Recover,
    Complete,
}

pub(super) struct Launcher {
    root: PathBuf,
    /// This engine's own generation and hop, present when it runs as a generation's engine.
    route: Option<(sugarsubstitute_contracts::installation::GenerationId, u8)>,
    /// Start only the accepted release; set when this engine is a fallback.
    accepted_only: bool,
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
    /// Newer accepted updater that must own startup, held pinned until it takes over.
    delegate: Option<Arc<std::sync::Mutex<Option<Box<setup_storage::PreparationImage>>>>>,
    choice: Choice,
    recovery_until: Option<Moment>,
    recovery_wake: Option<Instant>,
    update_failure: Option<Box<dyn Error>>,
    budget: Option<LauncherBudget>,
    /// The "Starting…" notice shown while this launch takes more than a moment.
    notice: Option<super::notice::Notice>,
    /// Native wake at the update cutoff, until that cutoff has been observed.
    update_wake: Option<Instant>,
    until: Option<Moment>,
    cleanup_until: Option<Moment>,
    wake_at: Option<Instant>,
    closing: bool,
    completed: bool,
    stopping: bool,
    cutoff_reported: bool,
    /// When the process exits regardless, once the cleanup cutoff has been reported.
    exit_at: Option<(Moment, Option<Instant>)>,
    /// Workers never returned by `exit_at`; this process exits without joining them.
    abandoned: bool,
    failure: Option<Box<dyn Error>>,
    finished: bool,
}

/// The update took too long to begin, so the accepted version starts instead.
#[derive(Debug)]
pub(super) struct LateUpdate;
impl std::fmt::Display for LateUpdate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .write_str("The prepared update did not start in time; starting the accepted release")
    }
}
impl Error for LateUpdate {}
impl Launcher {
    /// Allocate inert composition state; all filesystem and native creation runs on workers.
    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            root,
            route: None,
            accepted_only: false,
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
            delegate: None,
            choice: Choice::Installation,
            recovery_until: None,
            recovery_wake: None,
            update_failure: None,
            budget: None,
            notice: None,
            update_wake: None,
            until: None,
            cleanup_until: None,
            wake_at: None,
            closing: false,
            completed: false,
            stopping: false,
            cutoff_reported: false,
            exit_at: None,
            abandoned: false,
            failure: None,
            finished: false,
        }
    }
    /// A generation's launch engine first hands startup to the generation that should run.
    pub(super) fn engine(
        root: PathBuf,
        generation: sugarsubstitute_contracts::installation::GenerationId,
        hops: u8,
    ) -> Self {
        let mut launcher = Self::new(root);
        launcher.route = Some((generation, hops));
        launcher.choice = Choice::Route;
        launcher
    }
    /// A newer updater that took over owns startup; otherwise only the accepted release starts.
    fn delegated(&mut self, takeover: Takeover) {
        self.delegate = None;
        if takeover == Takeover::TookOver {
            self.completed = true;
            self.choice = Choice::Complete;
            return;
        }
        tracing::warn!(
            operation = "startup_engine_refused",
            ?takeover,
            "The newer updater did not take over; starting the accepted release"
        );
        self.update_failure
            .get_or_insert_with(|| "The newer updater did not take over startup".into());
        self.choice = Choice::Accepted;
    }
    fn routed(&mut self, route: setup_storage::Route) {
        match route {
            setup_storage::Route::Own => self.choice = Choice::Installation,
            setup_storage::Route::AcceptedOnly => {
                self.accepted_only = true;
                self.choice = Choice::Installation;
            }
            setup_storage::Route::Redirected => {
                self.completed = true;
                self.choice = Choice::Complete;
            }
        }
    }
    fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        if let Some(client) = &self.client {
            client.drain_completions(max)?;
        }
        self.advance_notice();
        if let Some(pending) = &mut self.job
            && let Some(completion) = pending.job.try_take_completion()
        {
            let task = pending.task;
            self.job = None;
            match completion.outcome {
                JobOutcome::Success(Work::Routed(route)) => self.routed(route),
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
                JobOutcome::Success(Work::Delegated(takeover)) => self.delegated(takeover),
                JobOutcome::Success(Work::Launched) => {
                    self.completed = true;
                    self.choice = Choice::Complete;
                }
                JobOutcome::Success(Work::Recovered(outcome)) => self.recovered(outcome)?,
                JobOutcome::Success(Work::Retired) => {
                    self.launch = None;
                    self.engine = None;
                }
                JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(cause) => self.failed(task, cause.into())?,
                other => self.failed(
                    task,
                    format!("Launcher operation ended with {:?}", other.kind()).into(),
                )?,
            }
        }
        self.advance_cached()?;
        self.stop_late_update()?;
        if !self.closing && self.recovery_until.is_some_and(|until| self.clock.now() >= until) {
            return Err("Startup trial recovery reached its original cutoff".into());
        }
        if !self.closing
            && !self.completed
            && self.until.is_some_and(|until| self.clock.now() >= until)
        {
            return Err("Launcher startup reached its original cutoff".into());
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
        self.begin_budget(LauncherBudget::new(client.now()?)?);
        self.work_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.cleanup_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?);
        self.client = Some(client);
        self.select_next()
    }
    /// Startup needs no native loop of its own; the pump builds one only once the notice is
    /// due, so a fast launch never creates any window.
    fn prepare_native_runtime(&mut self, max: NonZeroUsize) -> Result<StartupPreparationStep> {
        Ok(match self.advance(max)? {
            HeadlessStep::Active if self.notice_shown() => StartupPreparationStep::Ready,
            HeadlessStep::Active => StartupPreparationStep::Waiting,
            HeadlessStep::Shutdown => StartupPreparationStep::Closing,
        })
    }
    fn notice(&mut self) -> Option<sugarsubstitute_desktop::HeadlessNotice> {
        self.notice.as_mut().and_then(super::notice::Notice::wanted)
    }
    fn notice_surface(&mut self, surface: sugarsubstitute_desktop::NoticeSurface) {
        if let Some(notice) = &mut self.notice {
            notice.surface(surface);
        }
        // Admit its first preparation now; the pump reports the surface between wakes.
        self.advance_notice();
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.advance(max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        if self.cutoff_reported {
            return if self.abandoned { None } else { self.exit_at.and_then(|(_, wake)| wake) };
        }
        [
            self.wake_at,
            self.update_wake,
            self.notice.as_ref().and_then(super::notice::Notice::next_wake),
            self.recovery_wake,
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
        if self.abandoned {
            return Err(self.abandon());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        if self.job.is_some()
            || self.launch.is_some()
            || self.cached.is_some()
            || self.engine.is_some()
            || self.delegate.is_some()
            || self.notice_busy()
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
