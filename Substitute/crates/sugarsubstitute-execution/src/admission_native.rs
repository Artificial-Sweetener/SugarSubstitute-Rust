// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Transfer bound or stopped native reservations without consuming ordinary task capacity.

use super::*;
use crate::task::{Work, stop_reason_at};

impl Core {
    pub(crate) fn take_native_at(
        &self,
        now: sugarsubstitute_contracts::execution::Moment,
    ) -> Option<(QueuedJob, bool)> {
        let mut state = lock(&self.state);
        let index = state.queue.iter().position(|queued| {
            if let Work::Native(work) = &queued.work {
                (work.ready()
                    || (work.held_creation() && stop_reason_at(&queued.context, now).is_some()))
                    && (stop_reason_at(&queued.context, now).is_some()
                        || Self::has_capacity(&state, queued.context.spec().class))
            } else {
                false
            }
        })?;
        let queued = state.queue.remove(index)?;
        if let Some(record) = state.jobs.get_mut(&queued.context.job_id()) {
            record.phase = JobPhase::Running;
            record.retiring = stop_reason_at(&queued.context, now).is_some();
        }
        queued.job.running();
        let wake = self.capacity_ready(&mut state);
        drop(state);
        Some((queued, wake))
    }
}
