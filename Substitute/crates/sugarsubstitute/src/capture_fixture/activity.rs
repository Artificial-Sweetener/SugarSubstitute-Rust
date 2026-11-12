// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume real capture facts; each new demand permits one finite cleanup attempt.
use super::*;
impl Fixture {
    /// Drain bounded notices before acknowledging their private capture jobs.
    pub(super) fn advance(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        let Some(client) = self.client.clone() else { return Ok(HeadlessStep::Active) };
        let batch = client.drain_completions(max)?;
        let mut first = None;
        for notification in &batch.notifications {
            if notification.scope_id == ScopeId::new(1)? {
                self.demand.terminal(notification.job_id.get(), notification.kind);
            }
            if notification.scope_id == ScopeId::new(2)?
                && notification.job_id.get() == specification::START_JOB
            {
                self.start_terminal_seen = true;
                if self.closing {
                    self.demand.request();
                }
            }
        }
        // Capture owns its typed jobs. Taking one can remove its mailbox notice;
        // drain each admitted work notice before permitting that acknowledgement.
        if self.demand.can_observe_capture(self.start_admitted, self.start_terminal_seen) {
            let updates = self.capture.observe(&batch);
            // Preserve the original typed native cause before categorical diagnostics.
            for cause in self.capture.errors().skip(self.reported_capture_errors) {
                self.failures.record(Box::new(cause.clone()));
                self.reported_capture_errors += 1;
                first.get_or_insert("Capture retained an original failure");
            }
            for fact in updates.observations() {
                if matches!(fact, CaptureObservation::HandlerExited { .. }) {
                    self.demand.request();
                }
                if let Err(cause) = self.evidence.observe(*fact) {
                    self.failures.record(cause);
                    if first.is_none() {
                        first = Some("Capture observation failed");
                    }
                }
            }
            if updates.omitted_observation_count() != 0 {
                self.failures.record("Capture observations exceeded the bounded turn".into());
                first.get_or_insert("Capture observation inventory incomplete");
            }
        }
        if let Some(watch) = &mut self.rejected_watch
            && let Some(completion) = watch.try_take_completion()
        {
            self.rejected_watch = None;
            match completion.outcome {
                JobOutcome::Cancelled => {}
                JobOutcome::Failure(cause) => {
                    self.failures.record(Box::new(cause));
                    first.get_or_insert("Rejected watch retirement failed");
                }
                _ => {
                    self.failures.record("Rejected watch has unexpected terminal kind".into());
                    first.get_or_insert("Rejected watch terminal mismatch");
                }
            }
        }
        let now = client.now()?;
        if !self.closing && !self.registration_expired && now >= self.budget.until() {
            self.registration_expired = true;
            self.failures.record("Capture original registration deadline elapsed".into());
            first.get_or_insert("Capture registration expired");
        }
        if self.closing {
            if let Some(stop) = &mut self.stop {
                let (grace, total) = stop.due(now);
                if grace {
                    self.demand.request();
                }
                if total && self.capture.snapshot().state != CaptureState::Closed {
                    self.failures.record(
                        "Capture original shutdown deadline elapsed with retained ownership".into(),
                    );
                    first.get_or_insert("Capture shutdown remains uncertain");
                }
            }
            if let Err(cause) = self.close_once(&client) {
                self.failures.record(cause);
                first.get_or_insert("Capture close admission failed");
            }
        }
        if let Some(message) = first {
            return Err(message.into());
        }
        Ok(if self.evidence.registered && !self.closing {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
    /// Consume one cleanup demand even when admission rejects the finite attempt.
    fn close_once(&mut self, client: &ExecutionClient) -> Result {
        if !self.start_admitted
            || self.capture.snapshot().state == CaptureState::Closed
            || self.stop.as_ref().is_none_or(|s| s.total_observed)
            || !self.demand.take()
        {
            return Ok(());
        }
        let id = self.demand.next_job()?;
        let deadline =
            self.stop.as_ref().ok_or("Capture close budget missing")?.budget.settle_until;
        let scope = self.cleanup_scope.as_ref().ok_or("Capture cleanup scope missing")?;
        match self.capture.submit_close(client, scope, specification::finite(id, deadline, 4096)?) {
            Ok(()) => {
                self.demand.admitted(id);
                Ok(())
            }
            // A later real registration/close terminal or handler exit rearms demand.
            Err(CrashAdmissionError::Busy) => Ok(()),
            Err(cause) => Err(Box::new(cause)),
        }
    }
}
