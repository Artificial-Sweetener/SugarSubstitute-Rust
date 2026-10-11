// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep forwarding adoption and retirement on the existing owned native worker.
use super::*;
use crate::task;
use sugarsubstitute_contracts::execution::{ExecutionError, JobOutcome, MemoryClaim, PanicCause};
pub(crate) struct Active {
    work: ForwardWork,
    context: JobContext,
    started: Moment,
    lease: Option<Box<dyn ForwardLease>>,
    claim: Option<MemoryClaim>,
    initialized: bool,
    prepared: bool,
    outcome: Option<JobOutcome<FixtureForwardingReport, NativeObservationError>>,
    completed: bool,
}
impl Active {
    pub(crate) fn new(
        work: ForwardWork,
        context: JobContext,
        _core: &Arc<Core>,
        now: Moment,
    ) -> Self {
        let lease = lock(&work.shared.state).lease.take();
        Self {
            work,
            context,
            started: now,
            lease,
            claim: None,
            initialized: false,
            prepared: false,
            outcome: None,
            completed: false,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        self.initialized = true;
        crate::native::record_owner_panic(&mut self.outcome, cause);
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        let observed = if clock_available { core.clock.now() } else { now };
        if self.outcome.is_none()
            && let Some(reason) = task::stop_reason_at(&self.context, observed)
        {
            self.outcome = Some(task::stopped_outcome(reason));
        }
        if !self.initialized && self.outcome.is_none() {
            self.initialized = true;
            match self.context.memory().claim(MIN_FIXTURE_SESSION_BYTES) {
                Ok(claim) => self.claim = Some(claim),
                Err(cause) => {
                    self.outcome = Some(JobOutcome::Failure(NativeObservationError::new(
                        "forwarding_memory",
                        cause,
                    )))
                }
            }
            if self.outcome.is_none() {
                match &mut self.lease {
                    Some(lease) => match lease.prepare() {
                        Ok(handles) => {
                            self.prepared = true;
                            let observed = if clock_available { core.clock.now() } else { now };
                            if let Some(reason) = task::stop_reason_at(&self.context, observed) {
                                self.outcome = Some(task::stopped_outcome(reason));
                            }
                            let mut state = lock(&self.work.shared.state);
                            state.prepared = self.outcome.is_none();
                            state.handles = handles;
                            drop(state);
                            if self.outcome.is_none() && core.mailbox.signal_capacity() {
                                core.mailbox.deliver_wake();
                            }
                        }
                        Err(error) => self.outcome = Some(JobOutcome::Failure(error)),
                    },
                    None => self.outcome = Some(JobOutcome::Cancelled),
                }
            }
        }
        let state = lock(&self.work.shared.state);
        let retiring = state.stopped || (state.transferred && !state.pinned);
        let pinned = state.pinned;
        drop(state);
        if self.outcome.is_none() && retiring {
            self.outcome = Some(if self.prepared {
                JobOutcome::Success(FixtureForwardingReport {
                    correlation: self.work.correlation,
                    prepared: true,
                })
            } else {
                JobOutcome::Cancelled
            });
        }
        if self.outcome.is_none() || pinned {
            return;
        }
        lock(&self.work.shared.state).stopped = true;
        if let Some(lease) = &mut self.lease
            && let Err(error) = lease.retire()
        {
            crate::native::retain_retirement(&mut self.outcome, error.clone());
            core.mailbox.record_failure(ExecutionError::new(
                self.context.spec().operation,
                Box::new(error),
            ));
            core.mailbox.deliver_wake();
            return;
        }
        drop(self.lease.take());
        self.claim = None;
        let ended = if clock_available { core.clock.now() } else { now };
        self.completed = true;
        if let Some(outcome) = self.outcome.take() {
            self.work.job.finish_at(&self.context, Some(self.started), ended, outcome);
        }
    }
}
