// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence actual connect/read/write acceptance against close, cancellation and original deadlines.
use super::*;
use admission::Phase;
use sugarsubstitute_contracts::execution::JobPhase;
pub(crate) enum Dispatch<T> {
    Issued(T),
    Revoked,
    Stopped,
    Expired,
}
#[derive(Clone, Copy)]
pub(crate) enum Activity<'a> {
    Connect,
    Read(&'a ReadCommand),
    Write(&'a Command),
}
pub(crate) struct Issue<'a> {
    pub(crate) core: &'a Core,
    pub(crate) shared: &'a Shared,
    pub(crate) context: &'a JobContext,
    pub(crate) activity: Activity<'a>,
}
impl Issue<'_> {
    /// Keep only nonblocking native acceptance and partial custody adoption inside the final fence.
    pub(crate) fn execute<T>(
        self,
        native: impl FnOnce() -> Result<T, LifecyclePipeFailure>,
    ) -> Result<Dispatch<T>, LifecyclePipeFailure> {
        let at = match self.core.clock.observe_unlatched() {
            Ok(at) => at,
            Err(payload) => self.core.clock.fail_observation(payload),
        };
        let state = lock(&self.core.state);
        let stopping = state.stopping;
        let record = state.jobs.get(&self.shared.id);
        let original_stop = crate::symbol_admission::original_stop(&state, self.shared.id, at)
            .or_else(|| {
                crate::coordinator_entry::recovery_grant::original_stop(
                    self.core,
                    &state,
                    self.shared.recovery_grant.as_ref(),
                    at,
                )
            });
        let current = self.context.owner_id() == self.core.id
            && record.is_some_and(|r| {
                !r.retiring
                    && !matches!(r.phase, JobPhase::Terminal(_))
                    && r.scope_id == self.context.scope_id()
                    && r.spec.id == self.context.job_id()
            });
        let deadlines = record.and_then(|r| r.native_deadlines);
        let mut pipe = lock(&self.shared.state);
        drop(state);
        if let Activity::Write(command) = self.activity {
            let phase = pipe
                .write_admission
                .as_ref()
                .filter(|a| a.operation == command.operation)
                .map(|a| a.phase)
                .ok_or_else(|| failure("pipe_write_admission"))?;
            if matches!(phase, Phase::Revoked) {
                return Ok(Dispatch::Revoked);
            }
            if !matches!(phase, Phase::Queued) {
                return Err(failure("pipe_write_issue_phase"));
            }
        }
        if let Some(reason) = original_stop {
            return Ok(match reason {
                crate::task::Stop::TimedOut => Dispatch::Expired,
                _ => Dispatch::Stopped,
            });
        }
        if !current || stopping || pipe.stopped || self.context.cancellation_requested() {
            return Ok(Dispatch::Stopped);
        }
        if self.core.clock.failed()
            || pipe.stopped
            || self.context.cancellation_requested()
            || deadlines.is_none_or(|d| d.stopped())
        {
            return Ok(Dispatch::Stopped);
        }
        let deadline = match self.activity {
            Activity::Connect => {
                if !pipe.prepared || !pipe.enrolled || pipe.connected {
                    return Err(failure("pipe_connect_phase"));
                }
                Some(self.shared.original_deadline)
            }
            Activity::Read(command) => {
                if !pipe.connected || !pipe.read_busy {
                    return Err(failure("pipe_read_phase"));
                }
                match command.deadline {
                    ReadDeadline::Original(until) => Some(until),
                    ReadDeadline::Passive => {
                        if !pipe.authenticated {
                            return Err(failure("pipe_passive_identity"));
                        }
                        None
                    }
                }
            }
            Activity::Write(command) => Some(command.deadline),
        };
        if deadlines.is_some_and(|d| d.expired(at))
            || deadline.is_some_and(|until| until <= at)
            || self.context.deadline().is_some_and(|until| until <= at)
        {
            return Ok(Dispatch::Expired);
        }
        if let Activity::Write(command) = self.activity {
            let admission =
                pipe.write_admission.as_mut().ok_or_else(|| failure("pipe_write_admission"))?;
            if admission.operation != command.operation {
                return Err(failure("pipe_write_admission"));
            }
            admission.phase = Phase::Issuing(at);
        }
        let accepted = match native() {
            Ok(value) => value,
            Err(error) => {
                if let Activity::Write(_) = self.activity
                    && let Some(admission) = &mut pipe.write_admission
                {
                    admission.phase = Phase::Retired;
                }
                return Err(error);
            }
        };
        if let Activity::Write(command) = self.activity {
            let admission =
                pipe.write_admission.as_mut().ok_or_else(|| failure("pipe_write_admission"))?;
            admission.phase = Phase::Issued { at, eligible: true };
            pipe.write_slots -= 1;
            pipe.observations
                .push_back(Observation::WriteIssued { operation: command.operation, at });
        }
        Ok(Dispatch::Issued(accepted))
    }
}
