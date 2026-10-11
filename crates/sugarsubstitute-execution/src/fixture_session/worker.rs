// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Commit protocol effects only on the retained central native worker.
use super::*;
use crate::task;
use sugarsubstitute_contracts::execution::{
    ExecutionError, JobContext, JobOutcome, MemoryClaim, Moment, PanicCause,
};

pub(crate) struct Active {
    work: FixtureWork,
    lease: Option<Box<dyn ParentLease>>,
    context: JobContext,
    started: Moment,
    ready: Arc<Signal>,
    response: Arc<Signal>,
    claim: Option<MemoryClaim>,
    initialized: bool,
    outcome: Option<JobOutcome<FixtureSessionReport, NativeObservationError>>,
    completed: bool,
}
impl Active {
    pub(crate) fn new(
        mut work: FixtureWork,
        context: JobContext,
        core: &Arc<Core>,
        now: Moment,
    ) -> Self {
        let lease = work.lease.take();
        Self {
            work,
            lease,
            context,
            started: now,
            ready: Signal::new(core),
            response: Signal::new(core),
            claim: None,
            initialized: false,
            outcome: None,
            completed: false,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        self.initialized = true;
        state(&self.work.shared).stopped = true;
        crate::native::record_owner_panic(&mut self.outcome, cause);
    }
    fn error(&mut self, error: NativeObservationError) {
        self.outcome = Some(JobOutcome::Failure(error));
    }
    fn publish(&self, core: &Core) {
        // This Active remains a charged running job across the outside-lock wake.
        // Its terminal cannot publish until the supplied callback actually returns.
        if core.mailbox.signal_capacity() {
            core.mailbox.deliver_wake();
        }
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        let observed = if clock_available { core.clock.now() } else { now };
        if self.outcome.is_none()
            && let Some(reason) = task::stop_reason_at(&self.context, observed)
        {
            self.outcome = Some(task::stopped_outcome(reason));
        }
        if self.outcome.is_none() && state(&self.work.shared).stopped {
            self.outcome = Some(JobOutcome::Cancelled);
        }
        if !self.initialized && self.outcome.is_none() {
            self.initialized = true;
            match self.context.memory().claim(MIN_FIXTURE_SESSION_BYTES) {
                Ok(claim) => self.claim = Some(claim),
                Err(cause) => self.error(NativeObservationError::new("fixture_memory", cause)),
            }
            if self.outcome.is_none() {
                let Some(lease) = &mut self.lease else { return };
                match lease.start(self.ready.clone(), self.response.clone()) {
                    Ok(handles) => {
                        if let Some(reason) = task::stop_reason_at(&self.context, core.clock.now())
                        {
                            self.outcome = Some(task::stopped_outcome(reason));
                        }
                        let mut s = state(&self.work.shared);
                        s.handles = handles;
                        if self.outcome.is_none() {
                            s.prepared = true;
                            s.unread[0] = true;
                        }
                        drop(s);
                        if self.outcome.is_none() {
                            self.publish(core);
                        }
                    }
                    Err(error) => self.error(error),
                }
            }
        }
        if self.outcome.is_none() {
            self.protocol(core);
        }
        if self.outcome.is_none() {
            return;
        }
        let mut s = state(&self.work.shared);
        s.stopped = true;
        let pinned = s.pin;
        drop(s);
        if pinned {
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
            self.work.job.finish_at(&self.context, Some(self.started), ended, outcome);
        }
    }
    fn protocol(&mut self, core: &Core) {
        if let Some(reason) = task::stop_reason_at(&self.context, core.clock.now()) {
            self.outcome = Some(task::stopped_outcome(reason));
            return;
        }
        let Some(lease) = &self.lease else { return };
        if let Err(error) = lease.validate_observation() {
            self.error(error);
            return;
        }
        if let Some(reason) = task::stop_reason_at(&self.context, core.clock.now()) {
            self.outcome = Some(task::stopped_outcome(reason));
            return;
        }
        let mut changed = false;
        let mut s = state(&self.work.shared);
        if self.ready.completed() && !s.ready {
            s.ready = true;
            s.unread[1] = true;
            changed = true;
        }
        // Selecting under the same control lock linearizes this finite command
        // against control cancellation. Only successful SetEvent becomes a fact.
        let challenge = !s.stopped
            && !self.context.cancellation_requested()
            && s.challenge_requested
            && !s.challenge_committed;
        if challenge {
            s.challenge_committed = true;
        }
        drop(s);
        if challenge {
            let Some(lease) = &mut self.lease else { return };
            if let Err(error) = lease.challenge() {
                self.error(error);
                return;
            }
            state(&self.work.shared).challenge_sent = true;
        }
        let mut s = state(&self.work.shared);
        if self.response.completed() && !s.responded {
            if !s.challenge_sent {
                drop(s);
                self.error(NativeObservationError::new(
                    "fixture_unsolicited_response",
                    std::io::Error::other("Response preceded committed Challenge"),
                ));
                return;
            }
            s.responded = true;
            s.unread[2] = true;
            changed = true;
        }
        drop(s);
        if changed {
            self.publish(core);
        }
        // Challenge and the supplied wake are finite but may cross the deadline.
        // Sample before selecting the next effect, never undo an actual Challenge.
        if let Some(reason) = task::stop_reason_at(&self.context, core.clock.now()) {
            self.outcome = Some(task::stopped_outcome(reason));
            return;
        }
        let mut s = state(&self.work.shared);
        let release = !s.stopped
            && !self.context.cancellation_requested()
            && s.release_requested
            && !s.release_committed;
        if release {
            s.release_committed = true;
        }
        drop(s);
        if release {
            let Some(lease) = &mut self.lease else { return };
            if let Err(error) = lease.release() {
                self.error(error);
                return;
            }
            let mut s = state(&self.work.shared);
            s.release_sent = true;
            self.outcome = Some(JobOutcome::Success(FixtureSessionReport {
                correlation: self.work.correlation,
                ready: s.ready,
                challenge_sent: s.challenge_sent,
                responded: s.responded,
                release_sent: true,
            }));
        }
    }
}
