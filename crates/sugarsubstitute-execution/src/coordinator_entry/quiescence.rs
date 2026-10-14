// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence empty server admission atomically while retaining actual native and borrowed custody.
use super::*;
use sugarsubstitute_contracts::execution::{CancelControl, JobPhase};

impl CoordinatorEntryControl {
    /// Fence new server admission only when no authenticated route or observation awaits its owner.
    /// This takes Core-before-session locks, matching native issue admission, and samples no clock.
    /// Returns false while an actual peer, command, observation or undecided request remains.
    /// Idle native listening slots and already checked-retired passive frame loans may remain;
    /// their original claims, callbacks and native completion still require ordinary shutdown.
    /// No loan is dropped and no retirement or terminal proof is fabricated by this fence.
    /// # Errors
    /// Rejects client roles, unactivated sessions, stale jobs and already cancelling execution.
    pub fn try_close_when_idle(&self) -> Result<bool, EntryTransportError> {
        if !self.shared.server {
            return Err(EntryTransportError::InvalidPhase);
        }
        let core = self.shared.core.upgrade().ok_or(EntryTransportError::Closed)?;
        {
            let state = lock(&core.state);
            let record = state.jobs.get(&self.shared.job_id).ok_or(EntryTransportError::Closed)?;
            let mut entry = lock(&self.shared.state);
            if state.stopping
                || record.retiring
                || matches!(record.phase, JobPhase::Terminal(_))
                || self.job.cancel.is_requested()
                || record.scope_id != self.job.scope_id
                || record.spec != self.job.spec
                || record.native_deadlines.is_none_or(|slots| slots.stopped())
                || core.clock.failed()
            {
                return Err(EntryTransportError::Closed);
            }
            if !entry.prepared || entry.owner.is_none() {
                return Err(EntryTransportError::InvalidPhase);
            }
            if entry.activation.is_some()
                || !entry.observations.is_empty()
                || entry.requests.iter().any(Option::is_some)
                || entry.connections.iter().any(pending_connection)
            {
                return Ok(false);
            }
            entry.stopped = true;
        }
        core.changed();
        Ok(true)
    }
}

fn pending_connection(slot: &Connection) -> bool {
    // Only actual lease retirement clears id/peer/header while loan remains true.
    // Construction starts loan=false; abandonment never clears these identities.
    // A retired borrowed frame therefore keeps its funded custody without admitting I/O or reuse.
    slot.peer.is_some()
        || slot.header.is_some()
        || slot.until.is_some()
        || slot.read.is_some()
        || slot.reply.is_some()
        || slot.reading
        || slot.writing
        || (slot.loan && slot.id.is_some())
}
