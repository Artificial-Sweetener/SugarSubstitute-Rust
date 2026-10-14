// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revalidate an original authenticated entry loan inside final native admission.
use super::PreparedStartupSnapshot;
use crate::{
    admission::{Core, State},
    coordinator_entry::{EntryTransportError, Shared, VerifiedEntryPeer, protocol},
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    coordinator_entry::EntryRequestId,
    execution::{CancelControl, CounterDeadline, JobId, JobPhase, Moment},
    lifecycle::RunKey,
};

/// Constructor-private matching facts; this owns no frame, new claim or native authority.
/// Arc aliases retain only the already-admitted Entry session metadata.
#[derive(Clone)]
pub(crate) struct EntryAdmissionBinding {
    shared: Arc<Shared>,
    job: JobId,
    index: usize,
    request: EntryRequestId,
    peer: VerifiedEntryPeer,
    source: CounterDeadline,
    until: Moment,
    header: protocol::Header,
}

pub(super) fn project(
    snapshot: &PreparedStartupSnapshot,
) -> Result<EntryAdmissionBinding, EntryTransportError> {
    let launch = snapshot.launch.as_ref().ok_or(EntryTransportError::Authentication)?;
    if launch.frame.buffer.is_none() {
        return Err(EntryTransportError::Closed);
    }
    snapshot.body()?;
    let frame = protocol::decode(launch.frame.bytes())?;
    if !matches!(frame.payload, protocol::DecodedPayload::Launch(_))
        || frame.header.request != launch.request
        || frame.header.source != launch.source
        || frame.header.namespace != launch.peer.namespace()
    {
        return Err(EntryTransportError::Authentication);
    }
    Ok(EntryAdmissionBinding {
        shared: launch.frame.shared.clone(),
        job: launch.frame.shared.job_id,
        index: launch.frame.index,
        request: launch.request,
        peer: launch.peer,
        source: launch.source,
        until: launch.until,
        header: frame.header,
    })
}

#[derive(Clone)]
pub(crate) enum StartupAdmissionBinding {
    Entry(EntryAdmissionBinding),
    Registered(super::super::registered_startup::RegisteredSnapshot),
}
impl StartupAdmissionBinding {
    pub(crate) fn validate_current_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        run: RunKey,
        now: Moment,
    ) -> Result<Moment, EntryTransportError> {
        match self {
            Self::Entry(entry) => entry.validate_current_locked(core, state, run, now),
            Self::Registered(registered) => {
                registered.validate_current_locked(core, state, run, now)
            }
        }
    }
    pub(crate) fn validate_registered_owner_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        run: RunKey,
        now: Moment,
    ) -> Result<Option<Moment>, EntryTransportError> {
        match self {
            Self::Entry(entry) => entry.registered_owner_locked(core, state, run, now),
            Self::Registered(_) => Err(EntryTransportError::Authentication),
        }
    }
}
impl EntryAdmissionBinding {
    /// Validate the surviving real Root Entry owner; the old request loan is historical.
    fn registered_owner_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        run: RunKey,
        now: Moment,
    ) -> Result<Option<Moment>, EntryTransportError> {
        if self.shared.core.upgrade().is_none_or(|actual| !Arc::ptr_eq(&actual, core))
            || !self.shared.server
            || state.stopping
            || core.clock.failed()
        {
            return Err(EntryTransportError::Closed);
        }
        let record = state.jobs.get(&self.job).ok_or(EntryTransportError::Closed)?;
        if record.phase != JobPhase::Running
            || record.retiring
            || record.memory.is_none()
            || record.job.cancellation().is_requested()
            || record.native_deadlines.is_none_or(|deadlines| deadlines.stopped())
        {
            return Err(EntryTransportError::Closed);
        }
        let until = crate::coordinator_entry::registered_startup::admission::scope_current(
            state,
            record.scope_id,
        )?;
        let until = crate::scope::earliest(until, record.deadline);
        if until.is_some_and(|until| now >= until) {
            return Err(EntryTransportError::InvalidDeadline);
        }
        let entry = lock(&self.shared.state);
        if !entry.prepared
            || entry.stopped
            || entry.owner != Some(run.owner)
            || entry.activation.is_some_and(|pending| pending != run.owner)
            || self.header.namespace != run.owner.namespace
            || self.header.epoch != Some(run.owner.epoch)
            || self.peer.namespace() != run.owner.namespace
        {
            return Err(EntryTransportError::Authentication);
        }
        Ok(until)
    }
    /// Check the actual original Entry job and route before final admission consumes input.
    /// Caller holds this Core's state mutex and supplies its already observed clock sample.
    /// This method acquires only Entry state, calls no provider and performs no effects.
    /// Returns the effective narrowed cutoff for the finite admission and handshake;
    /// its original JobSpec and cross-process CounterDeadline remain unchanged.
    /// # Errors
    /// Rejects closed, foreign, cancelled, replaced, narrowed-expired or mismatched custody.
    pub(crate) fn validate_current_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        run: RunKey,
        now: Moment,
    ) -> Result<Moment, EntryTransportError> {
        let original_core = self.shared.core.upgrade().ok_or(EntryTransportError::Closed)?;
        if !Arc::ptr_eq(core, &original_core) || state.stopping || core.clock.failed() {
            return Err(EntryTransportError::Closed);
        }
        let record = state.jobs.get(&self.job).ok_or(EntryTransportError::Closed)?;
        if record.spec.id != self.job
            || record.phase != JobPhase::Running
            || record.retiring
            || record.job.cancellation().is_requested()
            || record.native_deadlines.is_none_or(|deadlines| deadlines.stopped())
        {
            return Err(EntryTransportError::Closed);
        }
        let mut effective = record.deadline.map_or(self.until, |deadline| self.until.min(deadline));
        let mut ancestor = Some(record.scope_id);
        // The same Core admits at most 64 ordinary plus 132 native scopes.
        let mut remaining = 64 + 132;
        while let Some(id) = ancestor {
            if remaining == 0 {
                return Err(EntryTransportError::Closed);
            }
            remaining -= 1;
            let scope = state.scopes.get(&id).ok_or(EntryTransportError::Closed)?;
            if scope.spec.id != id || scope.closed || scope.abandoned || scope.cancel.is_requested()
            {
                return Err(EntryTransportError::Closed);
            }
            if let Some(deadline) = scope.deadline {
                effective = effective.min(deadline);
            }
            ancestor = scope.parent;
        }
        if now >= effective {
            return Err(EntryTransportError::InvalidDeadline);
        }
        let entry = lock(&self.shared.state);
        if !self.shared.server || !entry.prepared || entry.stopped {
            return Err(EntryTransportError::Closed);
        }
        // Activation is consumed by the worker before it publishes the active owner.
        if entry.owner != Some(run.owner)
            || entry.activation.is_some_and(|pending| pending != run.owner)
            || self.peer.namespace() != run.owner.namespace
            || self.header.namespace != run.owner.namespace
            || self.header.epoch != Some(run.owner.epoch)
        {
            return Err(EntryTransportError::Authentication);
        }
        let slot = entry.connections.get(self.index).ok_or(EntryTransportError::Closed)?;
        if slot.closing
            || !slot.loan
            || slot.returned.is_some()
            || slot.id != Some(self.peer.connection())
            || slot.peer != Some(self.peer)
        {
            return Err(EntryTransportError::Closed);
        }
        let header = slot.header.ok_or(EntryTransportError::Authentication)?;
        if !header.same_source(self.header)
            || header.epoch != self.header.epoch
            || header.nonce != self.header.nonce
            || header.request != self.request
            || header.source != self.source
        {
            return Err(EntryTransportError::Authentication);
        }
        let until = slot.until.ok_or(EntryTransportError::InvalidDeadline)?;
        let route = entry
            .requests
            .iter()
            .flatten()
            .find(|route| {
                route.connection == self.peer.connection() && route.request == self.request
            })
            .ok_or(EntryTransportError::Closed)?;
        if route.peer != self.peer || route.reply.is_some() {
            return Err(EntryTransportError::Authentication);
        }
        if until > self.until || route.until > self.until || now >= until || now >= route.until {
            return Err(EntryTransportError::InvalidDeadline);
        }
        Ok(effective.min(until).min(route.until))
    }
}
