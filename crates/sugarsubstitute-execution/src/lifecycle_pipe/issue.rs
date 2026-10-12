// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize final native write acceptance against exact-operation revocation.
use super::*;
use admission::Phase;
pub(crate) enum WriteDispatch<T> {
    Issued(T),
    Revoked,
    Stopped,
    Expired,
}
pub(crate) struct WriteIssue<'a> {
    pub(crate) core: &'a Core,
    pub(crate) shared: &'a Shared,
    pub(crate) context: &'a JobContext,
    pub(crate) command: &'a Command,
}
impl WriteIssue<'_> {
    /// Execute only the nonblocking external issue and custody adoption under this fence.
    /// Completion polling and external diagnostic/wake delivery happen after it releases.
    pub(crate) fn execute<T>(
        self,
        native: impl FnOnce() -> Result<T, LifecyclePipeFailure>,
    ) -> Result<WriteDispatch<T>, LifecyclePipeFailure> {
        let state = lock(&self.core.state);
        let stopping = state.stopping;
        let deadlines = state.jobs.get(&self.shared.id).and_then(|record| record.native_deadlines);
        let mut pipe = lock(&self.shared.state);
        // Admission/revocation acquire Core first. Copy fixed timers, then release
        // Core before clocks or native effects while State preserves the issue fence.
        drop(state);
        let phase = pipe
            .write_admission
            .as_ref()
            .filter(|admission| admission.operation == self.command.operation)
            .map(|admission| admission.phase)
            .ok_or_else(|| failure("pipe_write_admission"))?;
        if matches!(phase, Phase::Revoked) {
            return Ok(WriteDispatch::Revoked);
        }
        if !matches!(phase, Phase::Queued) {
            return Err(failure("pipe_write_issue_phase"));
        }
        if stopping || pipe.stopped || self.context.cancellation_requested() {
            return Ok(WriteDispatch::Stopped);
        }
        let at = match self.core.clock.observe_unlatched() {
            Ok(at) => at,
            Err(payload) => {
                drop(pipe);
                self.core.clock.fail_observation(payload)
            }
        };
        if self.core.clock.failed() || pipe.stopped || self.context.cancellation_requested() {
            return Ok(WriteDispatch::Stopped);
        }
        if deadlines.is_none_or(|deadlines| deadlines.stopped()) {
            return Ok(WriteDispatch::Stopped);
        }
        if deadlines.is_some_and(|deadlines| deadlines.expired(at))
            || self.command.deadline <= at
            || self.context.deadline().is_some_and(|deadline| deadline <= at)
        {
            return Ok(WriteDispatch::Expired);
        }
        let admission =
            pipe.write_admission.as_mut().ok_or_else(|| failure("pipe_write_admission"))?;
        admission.phase = Phase::Issuing(at);
        let accepted = match native() {
            Ok(accepted) => accepted,
            Err(error) => {
                admission.phase = Phase::Retired;
                return Err(error);
            }
        };
        admission.phase = Phase::Issued { at, eligible: true };
        pipe.write_slots -= 1;
        pipe.observations.push_back(LifecyclePipeObservation::WriteIssued {
            operation: self.command.operation,
            at,
        });
        drop(pipe);
        Ok(WriteDispatch::Issued(accepted))
    }
}
