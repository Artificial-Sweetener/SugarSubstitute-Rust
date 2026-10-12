// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advance admitted pipe commands and retain storage until actual native retirement.
use super::*;
use sugarsubstitute_contracts::execution::{ExecutionError, JobOutcome, PanicCause};
#[path = "protocol.rs"]
mod protocol;
#[path = "write_revocation.rs"]
mod write_revocation;
pub(crate) struct Active {
    work: PipeWork,
    context: JobContext,
    started: Moment,
    lease: Option<Box<dyn PipeLease>>,
    signals: [Arc<Signal>; 3],
    initialized: bool,
    prepared: bool,
    enrolled: bool,
    connecting: bool,
    connected: bool,
    read: Option<(ReadCommand, buffers::Buffer)>,
    write: Option<Write>,
    outcome: Option<JobOutcome<LifecyclePipeReport, LifecyclePipeFailure>>,
    completed: bool,
    native_retired: bool,
}
impl Active {
    pub(crate) fn new(
        mut work: PipeWork,
        context: JobContext,
        core: &Arc<Core>,
        now: Moment,
    ) -> Self {
        let lease = work.lease.take();
        Self {
            work,
            context,
            started: now,
            lease,
            signals: std::array::from_fn(|_| Signal::new(core)),
            initialized: false,
            prepared: false,
            enrolled: false,
            connecting: false,
            connected: false,
            read: None,
            write: None,
            outcome: None,
            completed: false,
            native_retired: false,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        match &mut self.outcome {
            Some(JobOutcome::Failure(error)) => error.cause.retain_panic(cause),
            Some(JobOutcome::Panicked(_)) => {}
            outcome => *outcome = Some(JobOutcome::Panicked(cause.clone())),
        }
    }
    fn error(&mut self, core: &Core, error: LifecyclePipeFailure) {
        if error.requires_process_shutdown() {
            core.mailbox.suppress_current_thread();
            core.native_identity_unrestored.store(true, std::sync::atomic::Ordering::Release);
            core.shutdown();
            core.mailbox.record_failure(ExecutionError::new(
                self.context.spec().operation,
                Box::new(error.clone()),
            ));
            core.mailbox.deliver_wake();
        }
        control::retain_failure(&mut self.outcome, error);
    }
    fn stop(&mut self, core: &Core, now: Moment) -> bool {
        if self.outcome.is_some() {
            return true;
        }
        if let Some(reason) = crate::task::stop_reason_at(&self.context, now) {
            self.outcome = Some(crate::task::stopped_outcome(reason));
            return true;
        }
        let state = lock(&core.state);
        let s = lock(&self.work.shared.state);
        if s.stopped {
            self.outcome = Some(JobOutcome::Cancelled);
        } else if state
            .jobs
            .get(&self.context.job_id())
            .and_then(|r| r.native_deadlines.as_ref())
            .is_some_and(|slots| slots.expired(now))
        {
            self.outcome = Some(JobOutcome::TimedOut);
        }
        self.outcome.is_some()
    }
    fn publish(&self, core: &Core) {
        if core.mailbox.signal_capacity() {
            core.mailbox.deliver_wake();
        }
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        // Adopt transferred handles even when cancellation won before preparation.
        let peer = { lock(&self.work.shared.state).peer.take() };
        if let Some(peer) = peer
            && let Some(lease) = &mut self.lease
        {
            lease.adopt(peer);
            self.enrolled = true;
        }
        let observed = if clock_available { core.clock.now() } else { now };
        self.stop(core, observed);
        if !self.initialized && self.outcome.is_none() {
            self.initialized = true;
            if let Err(error) = allocate(&self.context, &self.work.shared) {
                self.error(core, error);
            }
        }
        let server = lock(&self.work.shared.state).run.is_some();
        if !self.prepared && self.outcome.is_none() && (server || self.enrolled) {
            if self.stop(core, core.clock.now()) {
                self.retirement(core, now, clock_available);
                return;
            }
            let result = self.lease.as_mut().ok_or_else(|| failure("pipe_missing_lease")).and_then(
                |lease| lease.prepare(self.signals.clone(), &self.context, self.work.handshake),
            );
            match result {
                Ok(inherited) => {
                    self.prepared = true;
                    let run = self.lease.as_ref().and_then(|lease| lease.run());
                    let deadline = self.lease.as_ref().and_then(|lease| lease.deadline());
                    let capability = self.lease.as_ref().and_then(|lease| lease.capability());
                    let mut s = lock(&self.work.shared.state);
                    s.inherited = inherited;
                    s.run = run;
                    s.prepared = true;
                    s.deadline = deadline;
                    s.capability = capability;
                    s.observations.push_back(LifecyclePipeObservation::Prepared);
                    drop(s);
                    self.publish(core);
                }
                Err(error) => self.error(core, error),
            }
        }
        if self.outcome.is_none()
            && self.prepared
            && self.enrolled
            && let Err(error) = self.protocol(core)
        {
            self.error(core, error);
        }
        if self.outcome.is_some() {
            self.retirement(
                core,
                now,
                clock_available
                    && !core.native_identity_unrestored.load(std::sync::atomic::Ordering::Acquire),
            );
        }
    }
    fn retirement(&mut self, core: &Core, now: Moment, clock_available: bool) {
        let (pinned, peer, abandoned, authentication, mut capability) = {
            let mut state = lock(&core.state);
            let mut s = lock(&self.work.shared.state);
            s.stopped = true;
            if let Some(slots) =
                state.jobs.get_mut(&self.context.job_id()).and_then(|r| r.native_deadlines.as_mut())
            {
                slots.stop();
            }
            let mut abandoned = std::collections::VecDeque::with_capacity(8);
            // Preserve passive actual facts through terminal; only buffer guards
            // require release before native/session settlement can complete.
            for _ in 0..s.observations.len() {
                if let Some(fact) = s.observations.pop_front() {
                    if matches!(
                        fact,
                        LifecyclePipeObservation::Received { .. }
                            | LifecyclePipeObservation::WriteRevoked { .. }
                    ) {
                        abandoned.push_back(fact);
                    } else {
                        s.observations.push_back(fact);
                    }
                }
            }
            (
                s.pin || s.input_pin,
                s.peer.take(),
                abandoned,
                s.authenticate.take(),
                s.capability.take(),
            )
        };
        // Publication releases input_pin under this same lock. Take the peer
        // with the pin snapshot so retirement cannot skip a late transfer.
        if let Some(peer) = peer {
            if let Some(lease) = &mut self.lease {
                lease.adopt(peer);
                self.enrolled = true;
            } else {
                lock(&self.work.shared.state).peer = Some(peer);
                self.error(core, failure("pipe_missing_lease"));
                return;
            }
        }
        // Guard destruction may take the session lock and release a real memory claim.
        drop(abandoned);
        drop(authentication);
        if let Some(capability) = &mut capability {
            capability.fill(0);
        }
        let result = if self.native_retired {
            Ok(true)
        } else {
            self.lease.as_mut().map_or(Ok(true), |lease| lease.retire(pinned))
        };
        match result {
            Ok(false) => return,
            Err(error) => {
                control::retain_failure(&mut self.outcome, error.clone());
                core.mailbox.record_failure(ExecutionError::new(
                    self.context.spec().operation,
                    Box::new(error),
                ));
                core.mailbox.deliver_wake();
                return;
            }
            Ok(true) => self.native_retired = true,
        }
        if let Some(bytes) = self.lease.as_mut().and_then(|lease| lease.take_completed_write())
            && let Some(write) = &self.write
        {
            if bytes as usize == write.length {
                let mut state = lock(&self.work.shared.state);
                state.written += 1;
                state.write_slots = 0;
                state.observations.push_back(LifecyclePipeObservation::Written {
                    operation: write.command.operation,
                    bytes,
                });
                drop(state);
                self.publish(core);
            } else {
                control::retain_failure(&mut self.outcome, failure("pipe_partial_write"));
            }
        }
        if lock(&self.work.shared.state).loans != 0 || pinned {
            return;
        }
        self.read = None;
        self.write = None;
        let (queued_read, queued_write, available_read, available_write) = {
            let mut s = lock(&self.work.shared.state);
            (s.read.take(), s.write.take(), s.read_buffer.take(), s.write_buffer.take())
        };
        drop((queued_read, queued_write, available_read, available_write));
        drop(self.lease.take());
        let ended = if clock_available { core.clock.now() } else { now };
        self.completed = true;
        if let Some(outcome) = self.outcome.take() {
            self.work.job.finish_at(&self.context, Some(self.started), ended, outcome);
        }
    }
}
