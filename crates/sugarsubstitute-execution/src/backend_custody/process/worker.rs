// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual finite creation across bounded protocol/native progress and cleanup.
use super::*;
use crate::{
    admission::Core,
    task::{self, Job},
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    ExecutionError, JobContext, JobOutcome, Moment, PanicCause,
};
type Factory = Box<
    dyn FnOnce(
            &JobContext,
            &CustodiedProcessMemory,
        )
            -> Result<Box<dyn CustodiedProcessDriver>, crate::native::NativeObservationError>
        + Send,
>;
pub(crate) struct Work {
    pub(super) factory: Option<Factory>,
    pub(super) job: Arc<Job<CustodiedProcessReport, CustodiedProcessFailure>>,
    pub(crate) until: Moment,
}
/// The actual Active Box is physically freed before its outside original charge.
pub(crate) struct Owner {
    active: Box<Active>,
    _memory: CustodiedProcessMemory,
}
struct Active {
    work: Box<Work>,
    driver: Option<Box<dyn CustodiedProcessDriver>>,
    context: JobContext,
    started: Moment,
    outcome: Option<JobOutcome<CustodiedProcessReport, CustodiedProcessFailure>>,
    initialized: bool,
    completed: bool,
    retirement_failed: bool,
    signal: Arc<crate::native::Signal>,
    memory: CustodiedProcessMemory,
}
/// Preflight both allocations and bounded outcome storage before admission transfers input.
pub(super) const fn storage_bytes() -> usize {
    size_of::<Work>()
        + size_of::<Active>()
        + size_of::<crate::native::Signal>()
        + 2 * size_of::<usize>()
}
impl Owner {
    pub(crate) fn new(
        work: Box<Work>,
        memory: CustodiedProcessMemory,
        context: JobContext,
        core: &Arc<Core>,
        now: Moment,
    ) -> Self {
        Self {
            active: Box::new(Active {
                work,
                driver: None,
                context,
                started: now,
                outcome: None,
                initialized: false,
                completed: false,
                retirement_failed: false,
                signal: crate::native::Signal::new(core),
                memory: memory.clone(),
            }),
            _memory: memory,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.active.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        self.active.initialized = true;
        match &mut self.active.outcome {
            Some(JobOutcome::Failure(first)) => first.cause.retain_panic(cause),
            _ => {
                let mut error = crate::native::NativeObservationError::new(
                    "custodied_creation_owner",
                    OwnerPanic(cause.clone()),
                );
                error.retain_panic(cause);
                self.active.outcome = Some(JobOutcome::Failure(CustodiedProcessFailure {
                    cause: error,
                    memory: self.active.memory.clone(),
                }));
            }
        }
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        self.active.progress(core, now, clock_available);
    }
}
impl Active {
    fn failure(&mut self, cause: crate::native::NativeObservationError) {
        match &mut self.outcome {
            Some(JobOutcome::Failure(first)) => first.cause.retain_cleanup(cause),
            None => {
                self.outcome = Some(JobOutcome::Failure(CustodiedProcessFailure {
                    cause,
                    memory: self.memory.clone(),
                }))
            }
            _ => {}
        }
    }
    fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        if self.outcome.is_none()
            && let Some(stop) = task::stop_reason_at(&self.context, now)
        {
            self.outcome = Some(task::stopped_outcome(stop));
        }
        if !self.initialized {
            self.initialized = true;
            if self.outcome.is_none() {
                if let Err(cause) = self.memory.check_effect() {
                    self.failure(crate::native::NativeObservationError::new(
                        "custodied_creation_admission",
                        cause,
                    ));
                } else if let Some(factory) = self.work.factory.take() {
                    match factory(&self.context, &self.memory) {
                        Ok(driver) => self.driver = Some(driver),
                        Err(cause) => self.failure(cause),
                    }
                }
                if let Some(driver) = &mut self.driver
                    && let Err(cause) = driver.initialize(
                        &self.context,
                        &self.memory,
                        CustodiedProcessWake {
                            signal: self.signal.clone(),
                            _memory: self.memory.clone(),
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
                Ok(CustodiedProcessProgress::Pending) => {}
                Ok(CustodiedProcessProgress::Started) => {
                    // A late ACK cannot authorize resume/publication after original controls close.
                    match self.memory.check_effect() {
                        Ok(_) => {
                            self.outcome = Some(JobOutcome::Success(CustodiedProcessReport {
                                memory: self.memory.clone(),
                            }))
                        }
                        Err(cause) => self.failure(crate::native::NativeObservationError::new(
                            "custodied_creation_post",
                            cause,
                        )),
                    }
                }
                Err(cause) => self.failure(cause),
            }
        }
        if self.outcome.is_none() || self.retirement_failed {
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
                        Box::new(CustodiedProcessFailure { cause, memory: self.memory.clone() }),
                    ));
                    core.mailbox.deliver_wake();
                    return;
                }
            }
        }
        drop(self.driver.take());
        drop(self.work.factory.take());
        let ended = if clock_available { core.clock.now() } else { now };
        self.completed = true;
        if let Some(outcome) = self.outcome.take() {
            self.work.job.finish_at(&self.context, Some(self.started), ended, outcome);
        }
    }
}
/// Actual started creation report retaining its unchanged family/control claim.
pub struct CustodiedProcessReport {
    memory: CustodiedProcessMemory,
}
impl CustodiedProcessReport {
    /// Retain original family storage through the actual registry ownership transfer.
    pub fn memory(&self) -> CustodiedProcessMemory {
        self.memory.clone()
    }
}
/// Original creation failure with its inseparable outside allocation guard.
#[derive(Clone)]
pub struct CustodiedProcessFailure {
    cause: crate::native::NativeObservationError,
    memory: CustodiedProcessMemory,
}
impl CustodiedProcessFailure {
    /// Borrow the genuine first cause and bounded later retirement causes.
    pub const fn cause(&self) -> &crate::native::NativeObservationError {
        &self.cause
    }
    /// Keep all escaped error storage charged by the same original creation.
    pub fn memory(&self) -> CustodiedProcessMemory {
        self.memory.clone()
    }
}
impl std::fmt::Debug for CustodiedProcessFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::fmt::Display for CustodiedProcessFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for CustodiedProcessFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Preserve the actual bounded supervisor payload without converting its source to text.
#[derive(Debug)]
struct OwnerPanic(PanicCause);
impl std::fmt::Display for OwnerPanic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}
impl std::error::Error for OwnerPanic {}
