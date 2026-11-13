// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Announce worker-loop return so shutdown observers wait on one completion wake.

use crate::{admission::Core, runtime::WorkerStatus, scope::lock};
use std::sync::{Arc, Mutex};

/// Execution-owned facility whose loop return is part of shutdown settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerKind {
    /// The asynchronous supervisor task and its retained job joins.
    Async,
    /// One CPU lane thread.
    Cpu,
    /// The native registration/retirement thread.
    #[cfg(any(windows, test))]
    Native,
}

/// Count live worker loops per facility.
#[derive(Default)]
pub(crate) struct WorkerCensus(Mutex<Running>);

#[derive(Default)]
struct Running {
    async_loops: usize,
    cpu_loops: usize,
    native_loops: usize,
}

impl Running {
    fn slot(&mut self, kind: WorkerKind) -> &mut usize {
        match kind {
            WorkerKind::Async => &mut self.async_loops,
            WorkerKind::Cpu => &mut self.cpu_loops,
            #[cfg(any(windows, test))]
            WorkerKind::Native => &mut self.native_loops,
        }
    }
}

impl WorkerCensus {
    /// Report each facility whose enlisted loops have all returned.
    pub(crate) fn status(&self) -> WorkerStatus {
        let running = lock(&self.0);
        WorkerStatus {
            native_finished: running.native_loops == 0,
            async_finished: running.async_loops == 0,
            cpu_finished: running.cpu_loops == 0,
        }
    }
}

/// Owned by one worker loop for its whole lifetime.
///
/// Workers take this as their first parameter so it drops after every other owned value,
/// including on unwind. The last drop publishes settlement, then delivers exactly one wake.
pub(crate) struct WorkerExit {
    core: Arc<Core>,
    kind: WorkerKind,
}

impl WorkerExit {
    /// Count a loop before its thread or task exists, so settlement cannot precede enlistment.
    pub(crate) fn enlist(core: &Arc<Core>, kind: WorkerKind) -> Self {
        *lock(&core.workers.0).slot(kind) += 1;
        Self { core: core.clone(), kind }
    }
}

impl Drop for WorkerExit {
    fn drop(&mut self) {
        let last = {
            let mut running = lock(&self.core.workers.0);
            let slot = running.slot(self.kind);
            *slot = slot.saturating_sub(1);
            running.async_loops == 0 && running.cpu_loops == 0 && running.native_loops == 0
        };
        // The census lock is released before waking, so a woken observer reads `finished`.
        if last {
            self.core.mailbox.deliver_wake();
        }
    }
}

#[cfg(test)]
#[path = "worker_exit_tests.rs"]
mod tests;
