// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep physical pin/source custody after one preparation terminal and before actual close.
use super::*;
use control::{CleanupTrigger, request};
use sugarsubstitute_contracts::execution::{ExecutionError, JobOutcome, PanicCause};

pub(crate) struct Active {
    work: AttachWork,
    context: JobContext,
    started: Moment,
    signal: Arc<Signal>,
    initialized: bool,
    prepared: bool,
    terminal: bool,
    outcome: Option<JobOutcome<CaptureAttachLaunchPin, CaptureAttachFailure>>,
    completed: bool,
}
impl Active {
    pub(crate) fn new(
        work: AttachWork,
        context: JobContext,
        core: &Arc<Core>,
        now: Moment,
    ) -> Self {
        lock(&work.shared.custody.state).active = true;
        Self {
            work,
            context,
            started: now,
            signal: Signal::new(core),
            initialized: false,
            prepared: false,
            terminal: false,
            outcome: None,
            completed: false,
        }
    }
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        if self.terminal {
            return;
        }
        match &mut self.outcome {
            Some(JobOutcome::Failure(CaptureAttachFailure::Native(error))) => {
                error.retain_panic(cause)
            }
            Some(JobOutcome::Failure(CaptureAttachFailure::Source(error))) => {
                error.0.retain_panic(cause)
            }
            Some(JobOutcome::Panicked(_)) => {}
            outcome => *outcome = Some(JobOutcome::Panicked(cause.clone())),
        }
    }
    fn failed(&mut self, core: &Core, error: NativeObservationError) {
        match &mut self.outcome {
            Some(JobOutcome::Failure(CaptureAttachFailure::Native(primary))) => {
                primary.retain_cleanup(error.clone());
            }
            Some(JobOutcome::Failure(CaptureAttachFailure::Source(primary))) => {
                primary.0.retain_cleanup(error.clone());
            }
            Some(JobOutcome::Panicked(panic)) => {
                let mut retained = error.clone();
                retained.retain_panic(panic);
                self.outcome = Some(JobOutcome::Failure(CaptureAttachFailure::Native(retained)));
            }
            None if !self.terminal => {
                self.outcome =
                    Some(JobOutcome::Failure(CaptureAttachFailure::Native(error.clone())));
            }
            _ => {}
        }
        core.mailbox
            .record_failure(ExecutionError::new(self.context.spec().operation, Box::new(error)));
        core.mailbox.deliver_wake();
        request(&self.work.shared.custody, CleanupTrigger::FailedPreparation);
    }
    fn stop(&mut self, now: Moment) {
        if self.work.shared.custody.stopped() {
            return;
        }
        if self.context.cancellation_requested() {
            if !self.terminal && self.outcome.is_none() {
                self.outcome = Some(JobOutcome::Cancelled);
            }
            request(&self.work.shared.custody, CleanupTrigger::Cancelled);
        } else if !self.prepared && now >= self.work.handshake {
            if self.outcome.is_none() {
                self.outcome = Some(JobOutcome::TimedOut);
            }
            request(&self.work.shared.custody, CleanupTrigger::FailedPreparation);
        }
        if self.outcome.is_some() {
            request(&self.work.shared.custody, CleanupTrigger::FailedPreparation);
        }
        // Core shutdown/cancellation already claimed cleanup before wake delivery.
    }
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        if !self.initialized {
            self.initialized = true;
            match self.context.memory().claim(16_384) {
                Ok(memory) => lock(&self.work.shared.custody.state).memory = Some(memory),
                Err(cause) => {
                    self.failed(core, NativeObservationError::new("capture_pin_memory", cause))
                }
            }
            // Source association commits the original admission even if cancellation
            // won before acquisition. Memory and the actual view are retained before
            // any external source callback or native boundary can fail or unwind.
            match self.work.source.observe(self.work.shared.custody.expected, &self.context) {
                Ok(endpoint) => lock(&self.work.shared.state).endpoint = Some(endpoint),
                Err(cause) if self.outcome.is_none() => {
                    self.outcome = Some(JobOutcome::Failure(CaptureAttachFailure::Source(cause)));
                }
                Err(cause) => self.failed(core, cause.0),
            }
            let observed = if clock_available { core.clock.now() } else { now };
            self.stop(observed);
            if self.outcome.is_none() && !self.work.shared.custody.stopped() {
                let endpoint = lock(&self.work.shared.state).endpoint;
                if let Some(endpoint) = endpoint
                    && let Err(cause) = self.work.lease.prepare(
                        endpoint,
                        &self.context,
                        self.signal.clone(),
                        &self.work.shared,
                    )
                {
                    self.failed(core, cause);
                }
            }
        }
        let observed = if clock_available { core.clock.now() } else { now };
        self.stop(observed);
        if !self.prepared && !self.work.shared.custody.stopped() && self.outcome.is_none() {
            match self.work.lease.progress() {
                Ok(Some(handles)) if self.work.lease.prepared() => {
                    let original = lock(&self.work.shared.state).endpoint;
                    match self.work.source.observe(self.work.shared.custody.expected, &self.context)
                    {
                        Ok(current) if Some(current) == original => {}
                        Ok(_) => self.failed(
                            core,
                            NativeObservationError::new(
                                "capture_source_changed",
                                std::io::Error::other("Original published source changed"),
                            ),
                        ),
                        Err(cause) => {
                            self.outcome =
                                Some(JobOutcome::Failure(CaptureAttachFailure::Source(cause)));
                            request(&self.work.shared.custody, CleanupTrigger::FailedPreparation);
                        }
                    }
                    // Recheck publication after the actual native boundary returned.
                    let observed = if clock_available { core.clock.now() } else { now };
                    self.stop(observed);
                    let mut state = lock(&core.state);
                    let cleanup = lock(&self.work.shared.custody.state);
                    if !cleanup.stopped
                        && !core.clock.failed()
                        && !state.stopping
                        && !self.context.cancellation_requested()
                        && observed < self.work.handshake
                    {
                        let mut pin = lock(&self.work.shared.state);
                        pin.handles = Some(handles);
                        pin.pinned = true;
                        self.prepared = true;
                        if let Some(slots) = state
                            .jobs
                            .get_mut(&self.context.job_id())
                            .and_then(|record| record.native_deadlines.as_mut())
                        {
                            slots.clear(crate::native_deadlines::Role::Handshake, 0);
                        }
                        self.outcome = Some(JobOutcome::Success(CaptureAttachLaunchPin {
                            shared: self.work.shared.clone(),
                            released: false,
                        }));
                    }
                }
                Ok(_) => {}
                Err(cause) => self.failed(core, cause),
            }
        }
        if self.prepared && !self.terminal {
            self.publish(observed);
        }
        if self.work.shared.custody.stopped() {
            self.retire(core, observed);
        }
    }
    fn publish(&mut self, now: Moment) {
        if self.terminal {
            return;
        }
        if let Some(outcome) = self.outcome.take() {
            self.terminal = true;
            self.work.job.finish_at(&self.context, Some(self.started), now, outcome);
        }
    }
    fn retire(&mut self, core: &Core, now: Moment) {
        let cleanup = self.work.shared.custody.snapshot();
        let total_due = lock(&core.state)
            .jobs
            .get(&self.context.job_id())
            .and_then(|record| record.native_deadlines.as_ref())
            .is_some_and(|deadlines| deadlines.capture_cleanup_due(now)[1]);
        if total_due {
            self.work.shared.publish_retirement(core, Err(CaptureAttachFailure::Uncertain));
            if !self.terminal {
                self.publish(now);
            }
        }
        let borrowed = lock(&self.work.shared.state).pinned;
        let native_retired = lock(&self.work.shared.custody.state).native_retired;
        if !native_retired {
            match self.work.lease.retire(borrowed, &cleanup) {
                Ok(true) => lock(&self.work.shared.custody.state).native_retired = true,
                Ok(false) => return,
                Err(cause) => {
                    self.failed(core, cause);
                    return;
                }
            }
        }
        let report = CaptureAttachRetirementReport {
            run: self.work.shared.custody.expected.run(),
            execution: self.work.shared.custody.execution,
            job: self.work.shared.custody.job,
        };
        if !lock(&self.work.shared.custody.state).source_retired {
            // No execution/owner/issue lock crosses this actual source callback.
            if let Err(cause) = self.work.source.native_retired(&report) {
                self.failed(core, cause.0);
                return;
            }
            lock(&self.work.shared.custody.state).source_retired = true;
        }
        self.work.shared.publish_retirement(core, Ok(report));
        self.work.shared.custody.release_memory();
        core.retire_native_custody(self.context.job_id());
        if !self.terminal {
            if self.outcome.is_none() {
                self.outcome = Some(JobOutcome::Cancelled);
            }
            self.publish(now);
        }
        self.completed = true;
    }
}
