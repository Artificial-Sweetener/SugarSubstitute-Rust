// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require actual I/O and peer retirement and returned passive loans before releasing charged storage.
use super::*;
impl Active {
    pub(super) fn retire_connection(
        &mut self,
        core: &Core,
        index: usize,
    ) -> Result<(), EntryTransportError> {
        let reuse = self.work.shared.server
            || !self.return_unsubmitted && self.connections[index].retry_at.is_some();
        if !self.connections[index].native_retired {
            match self.lease()?.retire_connection(index, reuse) {
                Ok(false) => return Ok(()),
                Err(error) => {
                    if let Some(original) = &mut self.connections[index].failure {
                        original.retain_cleanup(error.clone());
                    }
                    if error.requires_process_shutdown() {
                        return Err(error);
                    }
                    self.diagnostic(core, error);
                    return Ok(());
                }
                Ok(true) => self.connections[index].native_retired = true,
            }
        }
        let (connection, loan) = {
            let state = lock(&self.work.shared.state);
            (state.connections[index].id, state.connections[index].loan)
        };
        if let Some(connection) = connection {
            let failure = self.connections[index]
                .failure
                .as_ref()
                .filter(|cause| {
                    matches!(
                        cause,
                        EntryTransportError::PeerDisconnected(_)
                            | EntryTransportError::EndpointAbsent(_)
                    )
                })
                .cloned();
            {
                let mut state = lock(&self.work.shared.state);
                state.emit(
                    Some(index),
                    EntryTransportObservation::ConnectionRetired { connection, failure },
                )?;
                // The final fact and checked-retired layout become visible together.
                // Preserve the original frame loan until its owner actually returns it.
                let slot = &mut state.connections[index];
                slot.id = None;
                slot.peer = None;
                slot.header = None;
                slot.until = None;
                slot.read = None;
                slot.reply = None;
                slot.reading = false;
                slot.writing = false;
                slot.latest = 0;
                slot.reserved = 0;
            }
            self.connections[index].failure = None;
            self.report.retired_connections += 1;
            if core.mailbox.signal_capacity() {
                core.mailbox.deliver_wake();
            }
        }
        self.connections[index].pending = None;
        if loan {
            return Ok(());
        }
        if self.return_unsubmitted {
            if self.connections[index].retry_at.is_some_and(|at| core.clock.now() < at) {
                return Ok(());
            }
            self.connections[index].retry_at = None;
            self.outcome = Some(JobOutcome::Success(self.report));
            return Ok(());
        }
        if reuse {
            let slot = &mut self.connections[index];
            slot.phase = Phase::Available;
            slot.native_retired = false;
            slot.failure = None;
            lock(&self.work.shared.state).connections[index].closing = false;
            core.changed();
        } else if !self.work.shared.server {
            self.outcome = Some(JobOutcome::Success(self.report));
        }
        Ok(())
    }
    pub(super) fn retirement(&mut self, core: &Core, now: Moment, clock_available: bool) {
        let serving = self.work.shared.server && lock(&self.work.shared.state).owner.is_some();
        if let Err(error) = self.cleanup.begin(serving, now, clock_available) {
            self.error(core, error);
        }
        if self.cleanup.take_expiry(now, clock_available) {
            self.error(
                core,
                EntryTransportError::native(NativeObservationError::new(
                    "entry_cleanup_settlement_timeout",
                    std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Actual native entry retirement remains unsettled",
                    ),
                )),
            );
        }
        let cleanup_deadline = self.cleanup.deadline(now, clock_available);
        let mut timer_error = None;
        let mut timer_changed = false;
        let abandoned = {
            let mut core_state = lock(&core.state);
            if let Some(slots) = core_state
                .jobs
                .get_mut(&self.context.job_id())
                .and_then(|record| record.native_deadlines.as_mut())
            {
                if cleanup_deadline.is_some() {
                    match self.schedule.publish(slots, cleanup_deadline) {
                        Ok(changed) => timer_changed = changed,
                        Err(error) => timer_error = Some(error),
                    }
                } else {
                    timer_changed = !slots.stopped();
                    slots.stop();
                }
            }
            let mut state = lock(&self.work.shared.state);
            state.stopped = true;
            // Guard destruction takes this session lock, so drop abandoned loans after releasing it.
            let mut abandoned = std::collections::VecDeque::with_capacity(MAX_ENTRY_CONNECTIONS);
            let count = state.observations.len();
            for _ in 0..count {
                if let Some(envelope) = state.observations.pop_front() {
                    if matches!(
                        envelope.observation,
                        EntryTransportObservation::RequestAuthenticated { .. }
                    ) {
                        if let Some(index) = envelope.slot {
                            state.connections[index].observed -= 1;
                        }
                        abandoned.push_back(envelope);
                    } else {
                        state.observations.push_back(envelope);
                    }
                }
            }
            abandoned
        };
        drop(abandoned);
        if timer_changed {
            core.changed();
        }
        if let Some(error) = timer_error {
            self.error(core, error);
        }
        if !self.native_retired {
            match self.lease.as_mut().map_or(Ok(true), |lease| lease.retire()) {
                Ok(false) => return,
                Err(error) => {
                    self.error(core, error);
                    return;
                }
                Ok(true) => self.native_retired = true,
            }
        }
        let loans = { lock(&self.work.shared.state).connections.iter().any(|entry| entry.loan) };
        if loans {
            return;
        }
        for slot in &mut self.connections {
            slot.read = None;
            slot.write = None;
            slot.pending = None;
        }
        {
            let mut state = lock(&self.work.shared.state);
            for slot in &mut state.connections {
                slot.returned = None;
                slot.read = None;
                slot.reply = None;
            }
        }
        self.replay = None;
        if self.return_unsubmitted && matches!(self.outcome, Some(JobOutcome::Success(_))) {
            let transferred = self.source_fence_for_return(core);
            if let Err(cause) = transferred {
                self.error(core, cause);
            }
        }
        drop(self.work.input.take());
        drop(self.source_pending.take());
        drop(self.source_owner.take());
        drop(self.source_input_claim.take());
        drop(self.lease.take());
        let ended = if clock_available { core.clock.now() } else { now };
        self.completed = true;
        if let Some(mut outcome) = self.outcome.take() {
            if matches!(outcome, JobOutcome::Success(_)) {
                outcome = JobOutcome::Success(self.report);
            }
            self.work.job.finish_at(&self.context, Some(self.started), ended, outcome);
        }
    }
}
