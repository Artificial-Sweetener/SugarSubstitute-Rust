// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Return an admitted peak-byte reservation only after its final retained guard.

use crate::{admission::Core, scope::lock};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{CancelControl, JobId, MemoryRelease, ScopeId};

pub(crate) struct Release {
    pub(crate) core: Arc<Core>,
    pub(crate) scope_id: ScopeId,
    pub(crate) job_id: JobId,
}

impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        let (observation, detached) = {
            let mut state = lock(&self.core.state);
            let observation =
                state.jobs.get_mut(&self.job_id).and_then(|record| record.memory.take());
            #[cfg(any(windows, test))]
            let permanent =
                state.jobs.get(&self.job_id).is_some_and(|record| record.process_capture.is_some());
            #[cfg(not(any(windows, test)))]
            let permanent = false;
            let detached = if permanent { state.jobs.remove(&self.job_id) } else { None };
            (observation, detached)
        };
        // The weak record cannot prolong its control storage beyond byte reclaim.
        // Dropping it outside Core invokes no provider or final admitted lease.
        drop(observation);
        drop(detached);
        {
            let core = &self.core;
            let mut state = lock(&core.state);
            state.bytes -= bytes;
            let scope_released = if let Some(scope) = state.scopes.get_mut(&self.scope_id) {
                scope.bytes -= bytes;
                scope.bytes == 0 && scope.cancel.is_requested()
            } else {
                false
            };
            Core::retire_scopes(&mut state);
            let wake = core.capacity_ready(&mut state);
            // A canceled owner may be waiting for its final retained result guard, even
            // when no later admission requested memory. Reuse the same coalesced mailbox.
            let wake = wake
                || (scope_released && !state.stopping && core.select_capacity_wake(&mut state));
            drop(state);
            core.deliver_capacity_wake(wake);
            core.changed();
        }
    }
}
