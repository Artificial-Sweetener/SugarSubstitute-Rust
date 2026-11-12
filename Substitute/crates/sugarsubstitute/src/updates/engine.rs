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
            state: Arc::new(Mutex::new(State { phase, failure: None })),
            job: None,
            parent: None,
            client,
            work,
            cleanup,
            closing: None,
            next_scope: 5,
            succeeded: false,
        }
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
        let bytes = state.reservation(self.closing.is_some());
        drop(state);
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
    pub(super) fn settled(&self) -> Result<bool> {
        Ok(self.job.is_none()
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
