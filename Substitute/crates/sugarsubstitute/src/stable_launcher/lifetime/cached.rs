// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reauthenticate cached startup candidates and retire optional failures before accepted launch.
use super::*;
use crate::authority_clock::Receipt;
use sugarsubstitute_supervision::{
    setup_storage::SignedCandidate,
    update_preparation::{Progress, Worker},
};
#[cfg(test)]
mod tests;

/// Borrow the launcher's original executor and scopes; preparation creates no scheduler or process.
pub(super) struct Resources<'a> {
    pub client: &'a ExecutionClient,
    pub work: &'a Scope,
    pub cleanup: &'a Scope,
    pub jobs: &'a crate::job_sequence::JobSequence,
    pub until: Moment,
    pub shutdown: Option<Moment>,
    pub delegate: bool,
}

/// Candidate custody transfers only after the actual verification completion has been observed.
pub(super) enum Outcome {
    Pending,
    Candidate(Box<SignedCandidate>),
    Engine(Box<setup_storage::PreparationImage>),
    Unavailable,
}

pub(super) struct Cached {
    worker: Worker,
    receipt: Option<Receipt>,
    cleanup_until: Option<Moment>,
    wake_at: Option<Instant>,
    failure: Option<Box<dyn Error>>,
}
impl Cached {
    /// Adopt inert preparation; all filesystem and authority checks run on admitted jobs.
    pub(super) fn new(worker: Worker) -> Self {
        Self { worker, receipt: None, cleanup_until: None, wake_at: None, failure: None }
    }

    /// The original optional retirement cutoff also constrains launcher shutdown.
    pub(super) fn cleanup_until(&self) -> Option<Moment> {
        self.cleanup_until
    }
    /// Wake the native pump for a fixed cleanup interval, including without worker notifications.
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        self.wake_at
    }

    /// Retain the first optional-update cause and report it without gating accepted startup on it.
    fn record(&mut self, cause: Box<dyn Error>) {
        tracing::warn!(operation="startup_cached_update_failed", cause=%crate::failure_details::Details(cause.as_ref()), "Cached update is unavailable; accepted launch follows retirement");
        self.failure.get_or_insert(cause);
    }

    /// Stop preparation once; repeated shutdown can shorten but never renew retirement.
    fn close(&mut self, now: Moment, external: Option<Moment>) -> Result<()> {
        let until = self.cleanup_until.map(Ok).unwrap_or_else(|| {
            sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(now)
                .map(|budget| budget.settle_until)
        })?;
        let until = external.map_or(until, |limit| until.min(limit));
        if self.cleanup_until != Some(until) {
            self.wake_at =
                Instant::now().checked_add(until.checked_duration_since(now).unwrap_or_default());
            self.cleanup_until = Some(until);
        }
        self.worker.cancel();
        Ok(())
    }

    /// Poll typed outcomes before advancing. Missing/expired/corrupt cache never authorizes a trial.
    /// Optional failures must finish actual storage retirement before returning `Unavailable`.
    pub(super) fn advance(&mut self, resources: Resources<'_>) -> Result<Outcome> {
        let now = resources.client.now()?;
        if let Some(observed) = self.worker.poll() {
            let clock = self.receipt.take().and_then(|receipt| receipt.take_failure());
            let failed = !matches!(observed.outcome, JobOutcome::Success(()));
            if let Some(cause) = clock {
                self.record(Box::new(cause));
            }
            if let Some(cause) = observed.failure {
                self.record(Box::new(io::Error::other(cause)));
            }
            if let Some(cause) = observed.retirement_failure {
                return Err(Box::new(io::Error::other(cause)));
            }
            if failed {
                self.record(match observed.outcome {
                    JobOutcome::Failure(cause) => Box::new(io::Error::other(cause)),
                    other => format!("Cached preparation stopped: {:?}", other.kind()).into(),
                });
            }
            if self.failure.is_some() {
                self.close(now, resources.shutdown)?;
            }
        }
        if resources.shutdown.is_some() {
            self.close(now, resources.shutdown)?;
        }
        if !self.worker.pending() {
            match self.worker.progress() {
                Progress::Closed => return Ok(Outcome::Unavailable),
                Progress::Ready if self.cleanup_until.is_none() => {
                    return self
                        .worker
                        .take_candidate()
                        .map(Outcome::Candidate)
                        .map_err(Into::into);
                }
                Progress::ActivationReady if self.cleanup_until.is_none() && resources.delegate => {
                    return self.worker.take_activation().map(Outcome::Engine).map_err(Into::into);
                }
                Progress::ActivationReady => self.close(now, resources.shutdown)?,
                Progress::Unavailable | Progress::AlreadyPreparing | Progress::Decision(_) => {
                    self.close(now, resources.shutdown)?
                }
                _ => {}
            }
            let until = self.cleanup_until.unwrap_or(resources.until);
            if now >= until {
                return Err("Cached startup preparation reached its original cutoff".into());
            }
            match self.submit(&resources) {
                Ok(()) => {}
                Err(cause) if self.cleanup_until.is_none() => {
                    self.record(Box::new(cause));
                    self.close(now, resources.shutdown)?;
                    // Rejected work has no completion wake. Admit cleanup in this same turn.
                    self.submit(&resources)?;
                }
                Err(cause) => return Err(cause.into()),
            }
        }
        Ok(Outcome::Pending)
    }

    /// Admission owns the UTC receipt only after the original executor accepts this exact job.
    fn submit(&mut self, resources: &Resources<'_>) -> io::Result<()> {
        let scope = if self.cleanup_until.is_some() { resources.cleanup } else { resources.work };
        self.receipt = crate::updates::admission::submit(
            &mut self.worker,
            resources.client,
            scope,
            resources.jobs.allocate().map_err(io::Error::other)?,
            self.cleanup_until.unwrap_or(resources.until),
        )?;
        Ok(())
    }
}
