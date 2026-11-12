// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe child roles and retire inherited ownership on the same native worker.
use super::{child::*, *};
use crate::task;
use sugarsubstitute_contracts::execution::{
    ExecutionError, JobContext, JobOutcome, MemoryClaim, Moment, PanicCause,
};

pub(crate) struct Active {
    work: ChildWork,
    context: JobContext,
    started: Moment,
    challenge: Arc<Signal>,
    release: Arc<Signal>,
    lease: Option<Box<dyn ChildLease>>,
    claim: Option<MemoryClaim>,
    initialized: bool,
    responded: bool,
    outcome: Option<JobOutcome<FixtureChildReport, NativeObservationError>>,
    completed: bool,
}
impl Active {
    pub(crate) fn new(work: ChildWork, context: JobContext, core: &Arc<Core>, now: Moment) -> Self {
        let lease = work.binding.take();
        Self {
            work,
            context,
            started: now,
            challenge: Signal::new(core),
            release: Signal::new(core),
            lease,
            claim: None,
            initialized: false,
            responded: false,
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
                        "fixture_child_memory",
                        cause,
                    )))
                }
            }
            if self.outcome.is_none() {
                self.outcome = match &mut self.lease {
                    Some(lease) => lease
                        .start(self.challenge.clone(), self.release.clone())
                        .err()
                        .map(JobOutcome::Failure),
                    None => Some(JobOutcome::Cancelled),
                };
            }
            if self.outcome.is_none() {
                // Validation/registration only prepare ownership. This fresh stop
                // checkpoint selects Ready; cancellation after selection cannot
                // undo an actual successful native signal.
                if let Some(reason) = task::stop_reason_at(&self.context, core.clock.now()) {
                    self.outcome = Some(task::stopped_outcome(reason));
                } else if let Some(lease) = &mut self.lease {
                    self.outcome = lease.ready().err().map(JobOutcome::Failure);
                }
            }
        }
        if self.outcome.is_none() {
            let observed = if clock_available { core.clock.now() } else { now };
            if let Some(reason) = task::stop_reason_at(&self.context, observed) {
                self.outcome = Some(task::stopped_outcome(reason));
            } else {
                self.protocol(core);
            }
        }
        if self.outcome.is_none() {
            return;
        }
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
            self.work.job.finish_in_custody_at(&self.context, Some(self.started), ended, outcome);
        }
    }
    fn protocol(&mut self, core: &Core) {
        let Some(lease) = &mut self.lease else { return };
        if let Err(error) = lease.validate_observation() {
            self.outcome = Some(JobOutcome::Failure(error));
            return;
        }
        if let Some(reason) = task::stop_reason_at(&self.context, core.clock.now()) {
            self.outcome = Some(task::stopped_outcome(reason));
            return;
        }
        if self.challenge.completed() && !self.responded {
            if let Err(error) = lease.response() {
                self.outcome = Some(JobOutcome::Failure(error));
                return;
            }
            self.responded = true;
        }
        if self.release.completed() {
            self.outcome = Some(JobOutcome::Success(FixtureChildReport {
                correlation: self.work.correlation,
                inheritance_cleared: true,
                ready_signalled: true,
                response_signalled: self.responded,
                release_observed: true,
            }));
        }
    }
}
