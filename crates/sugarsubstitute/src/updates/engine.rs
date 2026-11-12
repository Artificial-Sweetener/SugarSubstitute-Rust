// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Continue one update campaign through actual engine probing and delegated preparation.
mod process;
mod state;
#[cfg(test)]
mod tests;
use super::parent::{self, Parent};
use crate::job_sequence::JobSequence;
use state::{Phase, State};
use std::{
    error::Error,
    io,
    sync::{Arc, Mutex, MutexGuard, TryLockError},
    time::Instant,
};
use sugarsubstitute_contracts::update::attempt::{
    AttemptOutcome, AttemptProgress,
    codec::{UpdateAttemptChannel, UpdateAttemptKey},
};
use sugarsubstitute_contracts::{
    execution::*,
    lifecycle::{CoordinatorKey, ShutdownBudget},
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::update_preparation::EngineUpdate;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Shares its session's executor and step deadlines; never owns the application's run exclusion.
pub(super) struct Engine {
    state: Arc<Mutex<State>>,
    job: Option<OwnedJob<(), Arc<io::Error>>>,
    parent: Option<Parent>,
    client: ExecutionClient,
    work: Scope,
    cleanup: Scope,
    closing: Option<ShutdownBudget>,
    next_scope: u64,
    succeeded: bool,
    /// The delegated preparation's channel until its helper's owner takes it.
    attempt: Option<parent::AttemptChannel>,
    /// What the delegated preparation reported.
    attempt_outcome: Option<AttemptOutcome>,
}
impl Engine {
    /// Adopt observed storage custody without touching disk or launching a process.
    pub(super) fn new(
        source: EngineUpdate,
        client: ExecutionClient,
        work: Scope,
        cleanup: Scope,
    ) -> Self {
        let phase = match source {
            EngineUpdate::Prepared(prepared) => Phase::Prepared(Some(prepared)),
            EngineUpdate::Accepted(image) => Phase::Image(Some(image)),
        };
        Self {
            state: Arc::new(Mutex::new(State { phase, failure: None, attempt: None })),
            job: None,
            parent: None,
            client,
            work,
            cleanup,
            closing: None,
            next_scope: 5,
            succeeded: false,
            attempt: None,
            attempt_outcome: None,
        }
    }

    /// Latest progress the delegated preparation reported.
    pub(super) fn attempt_progress(&self) -> Option<AttemptProgress> {
        self.parent.as_ref().and_then(Parent::attempt_progress)
    }

    /// What the delegated preparation reported, once its helper settled.
    pub(super) fn take_attempt_outcome(&mut self) -> Option<AttemptOutcome> {
        self.attempt_outcome.take()
    }

    /// Reserve the delegated preparation's channel and hand its pins to the storage step that
    /// composes the helper. Returns whether the step may run now.
    fn prepare_attempt(&mut self, jobs: &JobSequence) -> Result<bool> {
        if self.attempt.is_none() {
            let spec = JobSpec {
                id: jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::NativeObservation,
                deadline: None,
                reservation_bytes:
                    sugarsubstitute_execution::purpose_pipe::MIN_LIFECYCLE_PIPE_BYTES,
                replacement: None,
            };
            let attempt = std::num::NonZeroU64::new(spec.id.get()).ok_or("Attempt id absent")?;
            let channel =
                UpdateAttemptChannel::new(process::key()?, UpdateAttemptKey::new(attempt));
            let handshake = self.client.now()?.checked_add(parent::attempt_handshake())?;
            self.attempt = Some(parent::AttemptChannel::reserve(
                &self.client,
                &self.cleanup,
                spec,
                channel,
                handshake,
            )?);
        }
        let channel = self.attempt.as_mut().ok_or("Update attempt channel absent")?;
        channel.poll(self.client.now()?)?;
        let mut state = observed_state(&self.state)?;
        if state.attempt.is_none()
            && let Some((bootstrap, enrollment)) = channel.take_pins()
        {
            state.attempt =
                Some(sugarsubstitute_supervision::UpdateAttemptPins { bootstrap, enrollment });
        }
        Ok(state.attempt.is_some())
    }

    /// Observe each terminal before advancing; the process table receives the same completion batch.
    pub(super) fn advance(&mut self, batch: &CompletionBatch, jobs: &JobSequence) -> Result<()> {
        self.observe_job()?;
        self.observe_process(batch, jobs)?;
        if self.job.is_some() || self.parent.is_some() || self.succeeded {
            return Ok(());
        }
        let state = self.state()?;
        if matches!(state.phase, Phase::Empty) {
            return Ok(());
        }
        if self.closing.is_none() && matches!(state.phase, Phase::Input(_)) {
            drop(state);
            self.launch()?;
            return self.observe_process(batch, jobs);
        }
        let composing = self.closing.is_none() && matches!(state.phase, Phase::Image(_));
        let bytes = state.reservation(self.closing.is_some());
        drop(state);
        if composing && !self.prepare_attempt(jobs)? {
            return Ok(());
        }
        self.submit(jobs, bytes)
    }

    /// Cancel ordinary work once; cleanup remains observed in its separate live scope.
    pub(super) fn close(&mut self, budget: ShutdownBudget) -> Result<()> {
        if self.closing.is_some() {
            return Ok(());
        }
        self.closing = Some(budget);
        if let Some(job) = &self.job {
            job.cancel();
        }
        if let Some(channel) = &mut self.attempt {
            channel.close();
        }
        if let Some(parent) = &mut self.parent {
            parent.close(Some(budget)).map_err(|cause| cause as Box<dyn Error>)?;
        }
        Ok(())
    }

    /// Native grace and settlement wakes of a delegated helper; ordinary steps need no wake.
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        self.parent.as_ref().and_then(Parent::next_deadline)
    }

    /// A delegated preparation process exited successfully and all its native custody retired.
    pub(super) fn succeeded(&self) -> bool {
        self.succeeded
    }

    /// Cleanup finishes only after actual jobs, processes and durable trial custody retire.
    pub(super) fn settled(&mut self) -> Result<bool> {
        let channel = self.attempt.as_mut().is_none_or(parent::AttemptChannel::settled);
        Ok(channel
            && self.job.is_none()
            && self.parent.is_none()
            && matches!(self.state()?.phase, Phase::Empty))
    }

    /// Composition never waits on a storage mutex. A worker can hold it only while `job` is present.
    fn state(&self) -> Result<MutexGuard<'_, State>> {
        observed_state(&self.state)
    }

    /// The shared state survives cancellation of result delivery; it retains original errors too.
    fn submit(&mut self, jobs: &JobSequence, bytes: u64) -> Result<()> {
        let closing = self.closing;
        let deadline = match closing {
            Some(budget) => budget.settle_until,
            None => self.client.now()?.checked_add(super::STEP)?,
        };
        let state = Arc::clone(&self.state);
        let key = process::key()?;
        self.job = Some(self.client.submit_blocking(
            if closing.is_some() { &self.cleanup } else { &self.work },
            JobSpec {
                id: jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: if closing.is_some() {
                    JobClass::InfrastructureLifecycle
                } else {
                    JobClass::InfrastructureOrdinary
                },
                deadline: Some(deadline),
                reservation_bytes: bytes,
                replacement: None,
            },
            move |context| {
                let mut state = state.lock().unwrap_or_else(|poison| poison.into_inner());
                let result = if closing.is_some() {
                    state.retire(&context)
                } else {
                    state.advance(key, &context)
                };
                result.map_err(|cause| {
                    let cause = Arc::new(cause);
                    state.failure = Some(cause.clone());
                    cause
                })
            },
        )?);
        Ok(())
    }

    /// Take failures only after the worker stops, preserving causes even when delivery was cancelled.
    fn observe_job(&mut self) -> Result<()> {
        let Some(done) = self.job.as_mut().and_then(OwnedJob::try_take_completion) else {
            return Ok(());
        };
        self.job = None;
        if let Some(cause) = self.state()?.failure.take() {
            return Err(io::Error::other(cause).into());
        }
        match done.outcome {
            JobOutcome::Success(()) => Ok(()),
            JobOutcome::Cancelled if self.closing.is_some() => Ok(()),
            JobOutcome::Failure(cause) => Err(io::Error::other(cause).into()),
            other => Err(super::session::stopped("Updater engine storage", other)),
        }
    }
}

/// A worker panic is reported through its actual completion; poisoning must not prevent cleanup.
fn observed_state(state: &Mutex<State>) -> Result<MutexGuard<'_, State>> {
    match state.try_lock() {
        Ok(state) => Ok(state),
        Err(TryLockError::Poisoned(poison)) => Ok(poison.into_inner()),
        Err(TryLockError::WouldBlock) => Err("Updater storage is still busy".into()),
    }
}
