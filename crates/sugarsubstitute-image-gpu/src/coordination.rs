// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate queue custody with the surface configuration exclusivity required by wgpu.

use crate::{GpuError, adapter::check};
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::JobContext;

const CONTROL_POLL: Duration = Duration::from_millis(25);

#[derive(Default)]
struct State {
    readers: usize,
    configuration_pending: bool,
    configuration_active: bool,
}

/// Device-wide admission for queue submission/presentation and exclusive surface configuration.
/// A pending writer prevents new readers so a stream of image work cannot starve reconfiguration.
#[derive(Default)]
pub(crate) struct SubmissionGate {
    state: Mutex<State>,
    changed: Condvar,
}

/// Clonable ownership of one admitted reader. Clones share a single reader count so nested GPU
/// work can reuse custody without re-entering a gate whose writer intent may already be pending.
#[derive(Clone)]
pub(crate) struct SubmissionPermit {
    _lease: Arc<SubmissionLease>,
}

struct SubmissionLease {
    gate: Arc<SubmissionGate>,
}

/// Persistent writer intent. Retaining this after a nonblocking attempt blocks subsequent readers
/// while already-admitted native work retires.
pub(crate) struct ConfigurationIntent {
    gate: Arc<SubmissionGate>,
    armed: bool,
}

/// Exclusive surface-configuration custody. It can atomically downgrade to queue custody before
/// acquiring the first image from the newly configured swapchain.
pub(crate) struct ConfigurationPermit {
    gate: Arc<SubmissionGate>,
    active: bool,
}

pub(crate) enum ConfigurationAttempt {
    Ready(ConfigurationPermit),
    Deferred(ConfigurationIntent),
}

impl SubmissionGate {
    pub(crate) fn submission(
        self: &Arc<Self>,
        context: &JobContext,
    ) -> Result<SubmissionPermit, GpuError> {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            check(context)?;
            if !state.configuration_pending && !state.configuration_active {
                state.readers = state
                    .readers
                    .checked_add(1)
                    .ok_or_else(|| GpuError::invalid("GPU submission custody exhausted"))?;
                return Ok(SubmissionPermit {
                    _lease: Arc::new(SubmissionLease { gate: self.clone() }),
                });
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, CONTROL_POLL)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state = next;
        }
    }

    #[cfg(test)]
    pub(crate) fn try_submission(self: &Arc<Self>) -> Option<SubmissionPermit> {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.configuration_pending || state.configuration_active {
            return None;
        }
        state.readers = state.readers.checked_add(1)?;
        Some(SubmissionPermit { _lease: Arc::new(SubmissionLease { gate: self.clone() }) })
    }

    pub(crate) fn request_configuration(self: &Arc<Self>) -> Option<ConfigurationIntent> {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.configuration_pending || state.configuration_active {
            return None;
        }
        state.configuration_pending = true;
        Some(ConfigurationIntent { gate: self.clone(), armed: true })
    }
}

impl ConfigurationIntent {
    pub(crate) fn try_acquire(mut self) -> ConfigurationAttempt {
        let mut state = self.gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.readers != 0 {
            drop(state);
            return ConfigurationAttempt::Deferred(self);
        }
        state.configuration_pending = false;
        state.configuration_active = true;
        drop(state);
        self.armed = false;
        ConfigurationAttempt::Ready(ConfigurationPermit { gate: self.gate.clone(), active: true })
    }
}

impl ConfigurationPermit {
    pub(crate) fn defer(mut self) -> ConfigurationIntent {
        let gate = self.gate.clone();
        {
            let mut state = gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            debug_assert!(state.configuration_active);
            state.configuration_active = false;
            state.configuration_pending = true;
        }
        self.active = false;
        ConfigurationIntent { gate, armed: true }
    }

    pub(crate) fn downgrade(mut self) -> SubmissionPermit {
        let gate = self.gate.clone();
        {
            let mut state = gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            debug_assert!(state.configuration_active);
            state.configuration_active = false;
            state.readers += 1;
        }
        gate.changed.notify_all();
        self.active = false;
        SubmissionPermit { _lease: Arc::new(SubmissionLease { gate }) }
    }
}

impl Drop for SubmissionLease {
    fn drop(&mut self) {
        let mut state = self.gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        debug_assert!(state.readers != 0);
        state.readers -= 1;
        let wake = state.readers == 0;
        drop(state);
        if wake {
            self.gate.changed.notify_all();
        }
    }
}

impl Drop for ConfigurationIntent {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut state = self.gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        debug_assert!(state.configuration_pending);
        state.configuration_pending = false;
        drop(state);
        self.gate.changed.notify_all();
    }
}

impl Drop for ConfigurationPermit {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut state = self.gate.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        debug_assert!(state.configuration_active);
        state.configuration_active = false;
        drop(state);
        self.gate.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_configuration_blocks_new_work_and_downgrades_without_a_gap()
    -> Result<(), Box<dyn std::error::Error>> {
        let gate = Arc::new(SubmissionGate::default());
        let existing = gate
            .try_submission()
            .ok_or_else(|| std::io::Error::other("initial submission did not enter"))?;
        let intent = gate
            .request_configuration()
            .ok_or_else(|| std::io::Error::other("configuration intent did not enter"))?;
        assert!(gate.try_submission().is_none());
        assert!(gate.request_configuration().is_none());
        let intent = match intent.try_acquire() {
            ConfigurationAttempt::Deferred(intent) => intent,
            ConfigurationAttempt::Ready(_) => {
                return Err(std::io::Error::other("configuration passed an active reader").into());
            }
        };
        drop(existing);
        let exclusive = match intent.try_acquire() {
            ConfigurationAttempt::Ready(permit) => permit,
            ConfigurationAttempt::Deferred(_) => {
                return Err(std::io::Error::other("retired reader did not release writer").into());
            }
        };
        assert!(gate.try_submission().is_none());
        let intent = exclusive.defer();
        assert!(gate.try_submission().is_none());
        let exclusive = match intent.try_acquire() {
            ConfigurationAttempt::Ready(permit) => permit,
            ConfigurationAttempt::Deferred(_) => {
                return Err(std::io::Error::other("deferred writer lost exclusivity").into());
            }
        };
        let first_frame = exclusive.downgrade();
        let nested = first_frame.clone();
        assert!(gate.try_submission().is_some());
        let intent = gate
            .request_configuration()
            .ok_or_else(|| std::io::Error::other("next configuration did not enter"))?;
        drop(first_frame);
        let intent = match intent.try_acquire() {
            ConfigurationAttempt::Deferred(intent) => intent,
            ConfigurationAttempt::Ready(_) => {
                return Err(std::io::Error::other("nested readback lost submission custody").into());
            }
        };
        assert!(gate.try_submission().is_none());
        drop(nested);
        assert!(matches!(intent.try_acquire(), ConfigurationAttempt::Ready(_)));
        Ok(())
    }

    #[test]
    fn abandoned_configuration_intent_reopens_submission() -> Result<(), Box<dyn std::error::Error>>
    {
        let gate = Arc::new(SubmissionGate::default());
        let intent = gate
            .request_configuration()
            .ok_or_else(|| std::io::Error::other("configuration intent did not enter"))?;
        assert!(gate.try_submission().is_none());
        drop(intent);
        assert!(gate.try_submission().is_some());
        Ok(())
    }
}
