// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume exact owned typed completions without waiting on active native calls.
use super::*;
use sugarsubstitute_contracts::{
    crash::{CaptureFailureCode, CaptureOperation},
    execution::JobOutcome,
};

impl WindowsCrashCapture {
    /// Poll typed start/close/watch lifetimes after execution activity.
    ///
    /// Metadata is a wake hint only; actual identity/results come from retained
    /// OwnedJobs. Empty capacity wakes are valid. This method never blocks on
    /// unfinished native work, copies capture bytes or treats cancellation as exit.
    /// Original causes are retained independently of redacted categorical updates.
    pub fn observe(&mut self, _activity: &CompletionBatch) -> CaptureUpdates {
        let mut updates = CaptureUpdates::new();
        let start = self.start.as_mut().and_then(OwnedJob::try_take_completion);
        let close = self.close.as_mut().and_then(OwnedJob::try_take_completion);
        if let Some(completion) = start {
            // A skipped/cancelled factory still leaves bounded input in Shared.
            // Preserve its real admitted charge before terminal memory is dropped.
            if let Some(shared) = &self.shared {
                *lock(&shared.retained_memory) = Some(completion.memory.clone());
            }
            self.start = None;
            let healthy = matches!(completion.outcome, JobOutcome::Success(()));
            self.consume_work(completion.outcome, CaptureOperation::Registration, &mut updates);
            self.collect_native();
            self.pending_registration = healthy;
            if !healthy {
                self.snapshot.state = CaptureState::Degraded;
            }
        }
        if let Some(completion) = close {
            self.close = None;
            self.consume_work(completion.outcome, CaptureOperation::Retirement, &mut updates);
            self.collect_native();
        }
        if self.start.is_none() && self.close.is_none() {
            let terminal = self.watch.as_mut().and_then(OwnedJob::try_take_completion);
            if let Some(completion) = terminal {
                self.watch = None;
                match completion.outcome {
                    JobOutcome::Success(exit) => {
                        let actual = CaptureHandlerIdentity {
                            process_id: exit.process_id,
                            creation_time_100ns: exit.creation_time_100ns,
                        };
                        if self.snapshot.handler == Some(actual) {
                            self.snapshot.handler_exit = Some(exit.exit_code);
                            self.pending_registration = false;
                            if self.snapshot.state != CaptureState::Closing {
                                self.snapshot.state = CaptureState::Degraded;
                            }
                            updates.push(CaptureObservation::HandlerExited {
                                owner: self.owner,
                                handler: actual,
                                exit_code: exit.exit_code,
                            });
                        } else {
                            self.degraded(
                                CaptureOperation::Observation,
                                CaptureFailureCode::IdentityMismatch,
                                &mut updates,
                            );
                        }
                    }
                    JobOutcome::Failure(cause) => {
                        self.retain(CrashError::new(CaptureOperation::Observation, cause));
                        self.degraded(
                            CaptureOperation::Observation,
                            CaptureFailureCode::Native,
                            &mut updates,
                        );
                    }
                    JobOutcome::Panicked(cause) => {
                        self.retain(CrashError::new(
                            CaptureOperation::Observation,
                            std::io::Error::other(cause.as_str().to_owned()),
                        ));
                        self.degraded(
                            CaptureOperation::Observation,
                            CaptureFailureCode::Panicked,
                            &mut updates,
                        );
                    }
                    JobOutcome::TimedOut => self.degraded(
                        CaptureOperation::Observation,
                        CaptureFailureCode::TimedOut,
                        &mut updates,
                    ),
                    JobOutcome::Cancelled | JobOutcome::Coalesced { .. } => self.degraded(
                        CaptureOperation::Observation,
                        CaptureFailureCode::Cancelled,
                        &mut updates,
                    ),
                }
            }
            if self.pending_registration
                && self.watch.is_some()
                && self.snapshot.state != CaptureState::Closing
                && let Some(handler) = self.snapshot.handler
            {
                self.pending_registration = false;
                let deadline =
                    self.shared.as_ref().and_then(|shared| lock(&shared.native).deadline);
                let now = self
                    .execution
                    .as_ref()
                    .ok_or_else(|| CrashError::invalid(CaptureOperation::Registration))
                    .and_then(|execution| {
                        execution
                            .now()
                            .map_err(|cause| CrashError::new(CaptureOperation::Registration, cause))
                    });
                match now {
                    Ok(now) if deadline.is_some_and(|deadline| now < deadline) => {
                        self.snapshot.state = CaptureState::Registered;
                        updates.push(CaptureObservation::Registered { owner: self.owner, handler });
                    }
                    Ok(_) => self.degraded(
                        CaptureOperation::Registration,
                        CaptureFailureCode::TimedOut,
                        &mut updates,
                    ),
                    Err(cause) => {
                        self.retain(cause);
                        self.degraded(
                            CaptureOperation::Registration,
                            CaptureFailureCode::Panicked,
                            &mut updates,
                        );
                    }
                }
            }
            let retired = self.shared.as_ref().is_some_and(|shared| lock(&shared.native).retired);
            if retired && self.watch.is_none() && self.snapshot.state == CaptureState::Closing {
                self.snapshot.state = CaptureState::Closed;
                updates.push(CaptureObservation::Closed { owner: self.owner });
                self.shared = None;
            }
        }
        updates
    }
    fn collect_native(&mut self) {
        let Some(shared) = self.shared.clone() else {
            return;
        };
        let mut native = lock(&shared.native);
        self.snapshot.handler = native.handler;
        if let Some(deadline) = native.close_deadline {
            self.close_deadline =
                Some(self.close_deadline.map_or(deadline, |old| old.min(deadline)));
        }
        self.snapshot.callback_pin_bytes =
            native.lease.as_ref().map_or(0, |lease| lease.callback_pin_bytes());
        self.snapshot.callback_pin_reservation_bytes =
            native.lease.as_ref().map_or(0, |lease| lease.callback_pin_reservation_bytes());
        self.watch = self.watch.take().or_else(|| native.watch.take());
        for cause in native.errors.iter_mut().filter_map(Option::take) {
            self.retain(cause);
        }
        self.omitted = self.omitted.saturating_add(native.omitted);
        native.omitted = 0;
        if let Some(permit) = lock(&shared.permit).take() {
            self.watch = Some(permit.cancel_unbound());
        }
    }
    fn consume_work(
        &mut self,
        outcome: JobOutcome<(), CrashError>,
        operation: CaptureOperation,
        updates: &mut CaptureUpdates,
    ) {
        let category = match outcome {
            JobOutcome::Success(()) => return,
            JobOutcome::Failure(cause) => {
                self.retain(cause);
                CaptureFailureCode::Native
            }
            JobOutcome::Panicked(cause) => {
                self.retain(CrashError::new(
                    operation,
                    std::io::Error::other(cause.as_str().to_owned()),
                ));
                CaptureFailureCode::Panicked
            }
            JobOutcome::TimedOut => CaptureFailureCode::TimedOut,
            JobOutcome::Cancelled | JobOutcome::Coalesced { .. } => CaptureFailureCode::Cancelled,
        };
        self.degraded(operation, category, updates);
    }
    fn degraded(
        &mut self,
        operation: CaptureOperation,
        cause: CaptureFailureCode,
        updates: &mut CaptureUpdates,
    ) {
        self.pending_registration = false;
        if self.snapshot.state != CaptureState::Closing {
            self.snapshot.state = CaptureState::Degraded;
        }
        updates.push(CaptureObservation::Degraded { owner: self.owner, operation, cause });
    }
}
