// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supervise an optional preparation process independently of application and ComfyUI families.
mod cleanup;
mod continuation;
mod launch;
mod prepared;
use crate::job_sequence::JobSequence;
pub(super) use prepared::{LaunchScope, ProbeCustody};
use std::{error::Error, io, sync::Arc, time::Instant};
use sugarsubstitute_contracts::{
    execution::{
        CompletionBatch, JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob, Scope,
        ScopeSpec,
    },
    lifecycle::{CoordinatorKey, ProcessKey, ShutdownBudget, ShutdownId},
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::{
    RegistryError, RegistryObservation, WindowsLaunchInput, WindowsProcessRegistry,
    setup_storage::{self, RunningApplication},
};
type Failure = Box<dyn Error + Send + Sync>;
type Result<T> = std::result::Result<T, Failure>;

/// Move verified inputs and the empty process table off the composition thread at retirement.
struct Retained {
    source: Option<Arc<RunningApplication>>,
    input: Option<WindowsLaunchInput>,
    registry: Option<WindowsProcessRegistry>,
}

/// Shares the coordinator's executor, but owns distinct finite scopes and a helper-only registry.
pub(crate) struct Parent {
    client: ExecutionClient,
    work: Scope,
    cleanup: Scope,
    native: ScopeSpec,
    key: CoordinatorKey,
    retained: Option<Retained>,
    constructing: Option<OwnedJob<WindowsProcessRegistry, RegistryError>>,
    preparing: Option<OwnedJob<WindowsLaunchInput, io::Error>>,
    retiring: Option<OwnedJob<(), io::Error>>,
    retiring_application: bool,
    process: Option<ProcessKey>,
    /// Outer cutoff for a finite helper (an engine probe, or a continuation after close).
    /// A preparation helper has none: its own step deadlines and stall timeout bound its work.
    until: Option<Moment>,
    wake_at: Option<Instant>,
    shutdown: Option<ShutdownBudget>,
    continuation: Option<continuation::Window>,
    grace_wake: Option<Instant>,
    settle_wake: Option<Instant>,
    stop_issued: bool,
    retired: bool,
    failure: Option<Failure>,
    probe: bool,
    probe_custody: Option<ProbeCustody>,
    successful_exit: bool,
    completion_taken: bool,
}
impl Parent {
    /// Construct inert ownership after authenticated application readiness; admission happens in advance.
    pub(crate) fn new(
        client: ExecutionClient,
        work: Scope,
        cleanup: Scope,
        native: ScopeSpec,
        key: CoordinatorKey,
        source: Arc<RunningApplication>,
    ) -> Result<Self> {
        Ok(Self::owned(
            client,
            LaunchScope { work, cleanup, native, key },
            Retained { source: Some(source), input: None, registry: None },
            None,
        ))
    }

    /// Consume real worker/process observations. Optional update failures close only this owner.
    /// A failed execution clock is returned because safe deadline enforcement is then unavailable.
    pub(crate) fn advance(&mut self, batch: &CompletionBatch, jobs: &JobSequence) -> Result<()> {
        let now = self.client.now()?;
        if let Err(cause) = self.observe_construction() {
            self.record(cause);
        }
        if let Err(cause) = self.observe_preparation() {
            self.record(cause);
        }
        if let Err(cause) = self.observe_family(batch) {
            self.record(cause);
        }
        if let Err(cause) = self.observe_retirement() {
            self.record(cause);
        }
        if self.shutdown.is_none() {
            if self.failure.is_some() || self.until.is_some_and(|until| now >= until) {
                self.close(None)?;
            } else if let Err(cause) = self.launch(jobs) {
                self.record(cause);
                self.close(None)?;
            }
        }
        if self.shutdown.is_some()
            && let Err(cause) = self.retire(jobs, now)
        {
            self.record(cause);
        }
        if !self.retired
            && self.shutdown.is_some_and(|budget| now >= budget.settle_until)
            && self.settle_wake.take().is_some()
        {
            self.grace_wake = None;
            self.record(
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Preparation custody remains owned after its original cleanup cutoff",
                )
                .into(),
            );
        }
        Ok(())
    }

    /// Keep failures observable without turning a download failure into an application failure.
    fn record(&mut self, cause: Failure) {
        tracing::warn!(operation="update_helper_failed", cause=%crate::failure_details::Details(cause.as_ref()),
            "Background preparation could not complete");
        if self.failure.is_none() {
            self.failure = Some(cause);
        }
    }

    /// The outer cutoff wake of a finite helper, then native graceful and final cleanup cutoffs.
    /// A running preparation helper needs no wake: its completions arrive as execution events.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        if self.retired {
            return None;
        }
        if self.shutdown.is_none() {
            return self.wake_at;
        }
        let now = Instant::now();
        self.grace_wake
            .filter(|deadline| *deadline > now)
            .or_else(|| self.settle_wake.filter(|deadline| *deadline > now))
    }

    /// Complete only after native settlement and observed destruction of all preparation custody.
    pub(crate) fn settled(&self) -> bool {
        self.retired && self.probe_custody.is_none()
    }

    /// Conclude a settled application-derived helper as one check outcome, exactly once.
    /// Failures were already logged when recorded; a probe never reaches this owner.
    pub(crate) fn take_check_outcome(&mut self) -> Option<super::schedule::CheckOutcome> {
        let completion = self.take_completion()?;
        Some(if completion.succeeded && completion.probe.is_none() {
            super::schedule::CheckOutcome::Completed
        } else {
            super::schedule::CheckOutcome::Failed
        })
    }

    /// File verification may borrow a run guard; the running helper itself retains only image files.
    pub(crate) fn borrows_application(&self) -> bool {
        self.preparing.is_some()
            || self.retiring_application
            || self.retained.as_ref().is_some_and(|held| held.source.is_some())
    }

    /// One finite launch step, never beyond this helper's outer cutoff when it has one.
    fn step_deadline(&self) -> Result<Moment> {
        let step = self.client.now()?.checked_add(super::STEP)?;
        Ok(self.until.map_or(step, |until| until.min(step)))
    }

    /// Use the parent's shared monotone job sequence; rejected admission never reuses identities.
    fn spec(
        &self,
        jobs: &JobSequence,
        class: JobClass,
        bytes: u64,
        deadline: Option<Moment>,
    ) -> Result<JobSpec> {
        Ok(JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class,
            deadline,
            reservation_bytes: bytes,
            replacement: None,
        })
    }

    /// Adopt actual typed completions before advancing their dependent native operations.
    fn observe_construction(&mut self) -> Result<()> {
        if let Some(job) = &mut self.constructing
            && let Some(done) = job.try_take_completion()
        {
            self.constructing = None;
            match done.outcome {
                JobOutcome::Success(registry) => {
                    self.retained.as_mut().ok_or("Update custody absent")?.registry = Some(registry)
                }
                JobOutcome::Failure(cause) => return Err(cause.into()),
                JobOutcome::Cancelled if self.shutdown.is_some() => {}
                other => {
                    return Err(format!(
                        "Update registry construction stopped: {:?}",
                        other.kind()
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    /// A failed image must not prevent consuming the same batch for the separate process registry.
    fn observe_preparation(&mut self) -> Result<()> {
        if let Some(job) = &mut self.preparing
            && let Some(done) = job.try_take_completion()
        {
            self.preparing = None;
            match done.outcome {
                JobOutcome::Success(input) => {
                    self.retained.as_mut().ok_or("Update custody absent")?.input = Some(input)
                }
                JobOutcome::Failure(cause) => return Err(cause.into()),
                JobOutcome::Cancelled if self.shutdown.is_some() => {}
                other => {
                    return Err(
                        format!("Update image preparation stopped: {:?}", other.kind()).into()
                    );
                }
            }
        }
        Ok(())
    }

    /// Drain all cached observations before native retirement, including exit after a creation failure.
    fn observe_family(&mut self, batch: &CompletionBatch) -> Result<()> {
        if let Some(registry) = self.retained.as_mut().and_then(|held| held.registry.as_mut()) {
            registry.consume(batch)?;
            if let Some(process) = self.process {
                let mut exited = None;
                let mut failure = None;
                for _ in 0..5 {
                    match registry.take_observation(process)? {
                        Some(RegistryObservation::Failed { error, .. }) => {
                            failure = Some(error);
                        }
                        Some(RegistryObservation::RootExited { code, .. }) => {
                            exited = Some(code);
                            tracing::info!(
                                operation = "update_helper_exit",
                                code,
                                "Preparation process exited; family settlement follows"
                            );
                        }
                        Some(_) => {}
                        None => break,
                    }
                }
                if exited.is_some() {
                    self.successful_exit = exited == Some(0) && self.shutdown.is_none();
                    self.close(None)?;
                }
                if let Some(cause) = failure {
                    return Err(cause.into());
                }
                if let Some(code) = exited.filter(|code| *code != 0) {
                    return Err(
                        format!("Update preparation process failed with exit code {code}").into()
                    );
                }
            }
        }
        Ok(())
    }

    /// Typed destruction completion is observed independently of earlier optional failures.
    fn observe_retirement(&mut self) -> Result<()> {
        if let Some(job) = &mut self.retiring
            && let Some(done) = job.try_take_completion()
        {
            self.retiring = None;
            self.retiring_application = false;
            match done.outcome {
                JobOutcome::Success(()) => self.retired = true,
                JobOutcome::Failure(cause) => return Err(cause.into()),
                other => {
                    return Err(
                        format!("Update custody retirement stopped: {:?}", other.kind()).into()
                    );
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
