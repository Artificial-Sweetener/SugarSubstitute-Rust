// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Wake pending native admission retries after a previously rejected capacity becomes usable.

use super::*;

/// Retain bounded rejection interests, not callers, jobs or an additional queue.
#[derive(Default)]
pub(super) struct CapacityWait {
    queue: [bool; 3],
    completion: [bool; 3],
    memory: Option<u64>,
}

impl Core {
    /// Check and arm retry interests in the same transaction as rejected admission.
    pub(super) fn check_capacity(
        state: &mut State,
        spec: JobSpec,
        replaces_ready: bool,
    ) -> Result<(), AdmissionError> {
        Self::check_capacity_slots(state, spec, replaces_ready, 1)
    }
    pub(super) fn check_capacity_slots(
        state: &mut State,
        spec: JobSpec,
        replaces_ready: bool,
        slots: usize,
    ) -> Result<(), AdmissionError> {
        // An individually impossible request can never become admissible, so it is reported
        // as permanent before any transient refusal and registers no capacity interest.
        if spec.reservation_bytes > state.byte_limit {
            return Err(AdmissionError::ExceedsCapacity {
                requested_bytes: spec.reservation_bytes,
                limit_bytes: state.byte_limit,
            });
        }
        let ordinary = ordinary(spec.class);
        let native = spec.class == JobClass::NativeObservation;
        let index = if native { 2 } else { usize::from(ordinary) };
        let ready = state
            .queue
            .iter()
            .filter(|queued| (queued.context.spec().class == JobClass::NativeObservation) == native)
            .map(crate::task::QueuedJob::queue_slots)
            .sum::<usize>();
        let held = state
            .jobs
            .values()
            .filter(|record| {
                !record.process_lifetime_only()
                    && (record.spec.class == JobClass::NativeObservation) == native
            })
            .count();
        let queued_ordinary = state
            .queue
            .iter()
            .filter(|queued| super::ordinary(queued.context.spec().class))
            .map(crate::task::QueuedJob::queue_slots)
            .sum::<usize>();
        if !replaces_ready
            && (ready + slots > if native { 384 } else { 32 }
                || (ordinary && queued_ordinary + slots > 28))
        {
            state.capacity_wait.queue[index] = true;
            return Err(AdmissionError::QueueSaturated { class: spec.class });
        }
        let held_ordinary = state
            .jobs
            .values()
            .filter(|record| !record.process_lifetime_only() && super::ordinary(record.spec.class))
            .count();
        if held + slots > if native { 512 } else { 48 } || (ordinary && held_ordinary + slots > 44)
        {
            state.capacity_wait.completion[index] = true;
            return Err(AdmissionError::CompletionSaturated);
        }
        let available_bytes = state.byte_limit - state.bytes;
        if spec.reservation_bytes > available_bytes {
            state.capacity_wait.memory = Some(
                state
                    .capacity_wait
                    .memory
                    .map_or(spec.reservation_bytes, |waiting| waiting.min(spec.reservation_bytes)),
            );
            return Err(AdmissionError::MemorySaturated {
                requested_bytes: spec.reservation_bytes,
                available_bytes,
            });
        }
        Ok(())
    }

    /// Consume interests made usable by an actual release while the state is locked.
    ///
    /// Only the mailbox flag is changed here. Native wake delivery occurs after
    /// the caller releases the state lock, including releases from retained guards.
    pub(crate) fn capacity_ready(&self, state: &mut State) -> bool {
        if state.stopping {
            state.capacity_wait = CapacityWait::default();
            return false;
        }
        let wait = &state.capacity_wait;
        if wait.queue == [false; 3] && wait.completion == [false; 3] && wait.memory.is_none() {
            return false;
        }
        let queued_ordinary = state
            .queue
            .iter()
            .filter(|queued| ordinary(queued.context.spec().class))
            .map(crate::task::QueuedJob::queue_slots)
            .sum::<usize>();
        let held_ordinary = state
            .jobs
            .values()
            .filter(|record| !record.process_lifetime_only() && ordinary(record.spec.class))
            .count();
        let wait = &mut state.capacity_wait;
        let mut ready = false;
        for index in 0..3 {
            let native = index == 2;
            let ready_count = state
                .queue
                .iter()
                .filter(|queued| {
                    (queued.context.spec().class == JobClass::NativeObservation) == native
                })
                .map(crate::task::QueuedJob::queue_slots)
                .sum::<usize>();
            let held_count = state
                .jobs
                .values()
                .filter(|record| {
                    !record.process_lifetime_only()
                        && (record.spec.class == JobClass::NativeObservation) == native
                })
                .count();
            if wait.queue[index]
                && ready_count < if native { 384 } else { 32 }
                && (index != 1 || queued_ordinary < 28)
            {
                wait.queue[index] = false;
                ready = true;
            }
            if wait.completion[index]
                && held_count < if native { 512 } else { 48 }
                && (index != 1 || held_ordinary < 44)
            {
                wait.completion[index] = false;
                ready = true;
            }
        }
        if wait.memory.is_some_and(|bytes| bytes <= state.byte_limit - state.bytes) {
            wait.memory = None;
            ready = true;
        }
        ready && self.select_capacity_wake(state)
    }

    /// Keep a selected resource-activity callback owned through delivery or shutdown suppression.
    pub(crate) fn select_capacity_wake(&self, state: &mut State) -> bool {
        if !self.mailbox.signal_capacity() {
            return false;
        }
        match state.capacity_wakes_inflight.checked_add(1) {
            Some(inflight) => {
                state.capacity_wakes_inflight = inflight;
                true
            }
            None => {
                self.mailbox.record_failure(crate::runtime::error(std::io::Error::other(
                    "Capacity wake ownership count overflowed",
                )));
                false
            }
        }
    }

    /// Deliver only the coalesced capacity edge after the admission lock is released.
    ///
    /// Shutdown suppresses a callback not yet started. A callback already entered
    /// remains owned until it returns, so settlement cannot race UI disconnection.
    pub(crate) fn deliver_capacity_wake(&self, wake: bool) {
        if wake {
            if !lock(&self.state).stopping {
                self.mailbox.deliver_wake();
            }
            lock(&self.state).capacity_wakes_inflight -= 1;
            self.changed();
        }
    }
}

#[cfg(test)]
#[path = "admission_capacity_tests.rs"]
mod tests;
