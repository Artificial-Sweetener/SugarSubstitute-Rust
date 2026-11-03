// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one custody driver through real completion and checked native retirement.
use super::*;
use crate::{
    admission::Core,
    native::Signal,
    task::{self, Job},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    ExecutionError, JobContext, JobOutcome, Moment, PanicCause,
};

pub(crate) type Factory = Box<
    dyn FnOnce(
            &JobContext,
            &BackendCustodyMemory,
        )
            -> Result<Box<dyn BackendCustodyDriver>, crate::native::NativeObservationError>
        + Send,
>;
pub(crate) struct Work {
    pub(crate) binding: super::start::Binding,
    pub(crate) permit: BackendCustodyWorkerPermit,
    pub(crate) job: Arc<Job<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>>,
}
pub(crate) struct Active {
    // Every allocation and native owner precedes the inseparable outside claim.
    work: Box<Work>,
    driver: Option<Box<dyn BackendCustodyDriver>>,
    context: JobContext,
    signal: Arc<Signal>,
    started: Moment,
    outcome: Option<JobOutcome<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>>,
    initialized: bool,
    completed: bool,
    retirement_failed: bool,
    metadata: BackendCustodyMemory,
}
impl Active {
    /// Keep the custodian's renewable parent observation on its existing timer allocation.
    fn observe_parent_at(&self, core: &Core, at: Option<Moment>) {
        let changed = crate::scope::lock(&core.state)
            .jobs
            .get_mut(&self.context.job_id())
            .and_then(|record| record.custody_deadlines.as_mut())
            .is_some_and(|deadlines| deadlines.observe_parent_at(at));
        if changed {
            core.changed();
        }
    }
    pub(crate) fn new(
        work: Box<Work>,
        metadata: BackendCustodyMemory,
        context: JobContext,
        core: &Arc<Core>,
        now: Moment,
    ) -> Self {
        Self {
            work,
            driver: None,
            context,
            signal: Signal::new(core),
            started: now,
            outcome: None,
            initialized: false,
            completed: false,
            retirement_failed: false,
            metadata,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        if let Some(JobOutcome::Failure(first)) = &mut self.outcome {
            first.cause.retain_panic(cause);
        } else if self.outcome.is_none() {
            self.outcome = Some(JobOutcome::Panicked(cause.clone()));
        }
    }
    fn failure(&mut self, cause: crate::native::NativeObservationError) {
        match &mut self.outcome {
            Some(JobOutcome::Failure(first)) => first.cause.retain_cleanup(cause),
            None => {
                self.outcome = Some(JobOutcome::Failure(BackendCustodyWorkerFailure {
                    cause,
                    _metadata: self.metadata.clone(),
                }))
            }
            _ => {}
        }
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        if self.outcome.is_none()
            && let Some(reason) = task::stop_reason_at(&self.context, now)
        {
            self.outcome = Some(task::stopped_outcome(reason));
        }
        if !self.initialized {
            self.initialized = true;
            if self.outcome.is_none() {
                if let Err(cause) = self.work.permit.validate_current_admission() {
                    self.failure(cause);
                } else if let Some(factory) = self.work.binding.take() {
                    match factory(&self.context, &self.metadata) {
                        Ok(driver) => self.driver = Some(driver),
                        Err(cause) => self.failure(cause),
                    }
                }
                if let Some(driver) = &mut self.driver
                    && let Err(cause) = driver.initialize(
                        &self.context,
                        &self.metadata,
                        BackendCustodyWake {
                            signal: self.signal.clone(),
                            job: self.context.job_id(),
                        },
                    )
                {
                    self.failure(cause);
                }
            }
        }
        if self.outcome.is_none()
            && let Some(driver) = &mut self.driver
        {
            match driver.progress(&self.context, now) {
                Ok(BackendCustodyProgress::Pending) => self.observe_parent_at(core, None),
                Ok(BackendCustodyProgress::ObserveParentAt(at)) => {
                    self.observe_parent_at(core, Some(at))
                }
                Ok(BackendCustodyProgress::Settled) => {
                    self.outcome = Some(JobOutcome::Success(BackendCustodyWorkerReport {
                        key: self.work.permit.key(),
                        _metadata: self.metadata.clone(),
                    }));
                }
                Err(cause) => self.failure(cause),
            }
        }
        if self.outcome.is_none() {
            return;
        }
        if self.retirement_failed {
            return;
        }
        if let Some(driver) = &mut self.driver {
            match driver.retire(&self.context, now) {
                Ok(false) => return,
                Ok(true) => {}
                Err(cause) => {
                    self.retirement_failed = true;
                    self.failure(cause.clone());
                    core.mailbox.record_failure(ExecutionError::new(
                        self.context.spec().operation,
                        Box::new(BackendCustodyWorkerFailure {
                            cause,
                            _metadata: self.metadata.clone(),
                        }),
                    ));
                    core.mailbox.deliver_wake();
                    return;
                }
            }
        }
        drop(self.driver.take());
        drop(self.work.binding.take());
        let ended = if clock_available { core.clock.now() } else { now };
        self.completed = true;
        if let Some(outcome) = self.outcome.take() {
            self.work.job.finish_in_custody_at(&self.context, Some(self.started), ended, outcome);
        }
    }
}

/// Checked end of the one actual custody worker; retained metadata stays charged.
pub struct BackendCustodyWorkerReport {
    pub(crate) key: sugarsubstitute_contracts::backend_custody::BackendCustodyKey,
    pub(crate) _metadata: BackendCustodyMemory,
}
impl BackendCustodyWorkerReport {
    /// Return the immutable purpose-specific owner/session facts.
    pub const fn key(&self) -> sugarsubstitute_contracts::backend_custody::BackendCustodyKey {
        self.key
    }
}
/// Retain the original actual failure before its unchanged metadata guard.
#[derive(Clone)]
pub struct BackendCustodyWorkerFailure {
    pub(crate) cause: crate::native::NativeObservationError,
    pub(crate) _metadata: BackendCustodyMemory,
}
impl std::fmt::Debug for BackendCustodyWorkerFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::fmt::Display for BackendCustodyWorkerFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for BackendCustodyWorkerFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
