// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit closed commands under Core-before-session locks without waiting for native work.
use super::*;
use sugarsubstitute_contracts::{
    coordinator_entry::*,
    execution::{CancelControl, JobPhase, OwnedJob},
    lifecycle::CoordinatorKey,
};

/// Retain a persistent server and its observed actual retirement result.
pub struct CoordinatorEntryServerSession {
    /// Typed bounded commands and immutable actual observations.
    pub control: CoordinatorEntryControl,
    /// Terminal follows actual I/O/callback/handle retirement and returned frame loans.
    pub completion: OwnedJob<EntryTransportReport, EntryTransportError>,
}
/// Retain one client request through its actual native retirement result.
pub struct CoordinatorEntryClientSession {
    /// Typed bounded commands and closed correlated observations.
    pub control: CoordinatorEntryControl,
    /// Cancellation is observed separately from actual I/O and native retirement.
    pub completion: OwnedJob<EntryTransportReport, EntryTransportError>,
}
/// Queue bounded entry commands without exposing native handles, codec or scheduler access.
pub struct CoordinatorEntryControl {
    pub(super) shared: Arc<Shared>,
    pub(super) job: Arc<Job<EntryTransportReport, EntryTransportError>>,
}
impl CoordinatorEntryControl {
    fn commit<R>(
        &self,
        change: impl FnOnce(&mut State, Moment) -> Result<R, EntryTransportError>,
    ) -> Result<R, EntryTransportError> {
        let core = self.shared.core.upgrade().ok_or(EntryTransportError::Closed)?;
        let now = core.clock.now();
        let result = {
            let state = lock(&core.state);
            let record = state.jobs.get(&self.shared.job_id).ok_or(EntryTransportError::Closed)?;
            let mut entry = lock(&self.shared.state);
            if state.stopping
                || record.retiring
                || matches!(record.phase, JobPhase::Terminal(_))
                || entry.stopped
                || self.job.cancel.is_requested()
            {
                return Err(EntryTransportError::Closed);
            }
            change(&mut entry, now)
        };
        if result.is_ok() {
            core.changed();
        }
        result
    }
    /// Request worker activation with matching namespace and the unchanged original preparation.
    /// Root must validate its actual retained election/claim proof before this metadata operation.
    /// # Errors
    /// Rejects changed preparation, expiry, wrong namespace, duplicate activation or closed sessions.
    pub fn activate_server(
        &self,
        owner: CoordinatorKey,
        original_preparation: Moment,
    ) -> Result<(), EntryTransportError> {
        self.commit(|state, now| {
            if !self.shared.server
                || !state.prepared
                || state.owner.is_some()
                || state.activation.is_some()
            {
                return Err(EntryTransportError::InvalidPhase);
            }
            if owner.namespace != self.shared.facts.namespace() {
                return Err(EntryTransportError::Authentication);
            }
            if original_preparation != self.shared.original_preparation
                || now >= original_preparation
            {
                return Err(EntryTransportError::InvalidDeadline);
            }
            state.activation = Some(owner);
            Ok(())
        })
    }
    /// Take one actual observation and return its fixed inventory credit without waiting.
    pub fn take_observation(&self) -> Option<EntryTransportObservation> {
        let result = {
            let mut state = lock(&self.shared.state);
            let envelope = state.observations.pop_front()?;
            if let Some(index) = envelope.slot {
                state.connections[index].observed -= 1;
            }
            envelope.observation
        };
        self.shared.changed();
        Some(result)
    }
    /// Rearm the coalesced host wake only while this session has queued facts.
    /// Call after a bounded observation drain. Closed or terminal sessions may
    /// still retain facts and charged loans; this operation grants no native
    /// admission and samples no clock. Returns whether facts remained at the
    /// queue check, not whether a wake succeeded or native custody settled.
    /// The queue check and passive mailbox flag are atomic with respect to this
    /// session's readers. The supplied wake runs after both locks are released;
    /// its original failure remains observable through completion draining.
    pub fn rearm_observations(&self) -> bool {
        let Some(core) = self.shared.core.upgrade() else { return false };
        let wake = {
            let state = lock(&self.shared.state);
            if state.observations.is_empty() {
                return false;
            }
            core.mailbox.signal_capacity()
        };
        if wake {
            core.mailbox.deliver_wake();
        }
        true
    }
    /// Queue one closed protocol read with its unchanged absolute probe deadline.
    /// # Errors
    /// Rejects stale IDs, occupied read/loan slots, exhausted credits, expired or excessive deadlines.
    pub fn read(
        &self,
        connection: EntryConnectionId,
        operation: EntryOperationId,
        original_deadline: Moment,
    ) -> Result<(), EntryTransportError> {
        self.commit(|state, now| {
            let index = state.index(connection)?;
            validate(&state.connections[index], operation, original_deadline, now)?;
            if state.connections[index].reading
                || state.connections[index].loan
                || state.connections[index].read.is_some()
            {
                return Err(EntryTransportError::Busy);
            }
            state.reserve(index, 1)?;
            let entry = &mut state.connections[index];
            entry.latest = operation.get();
            entry.read = Some(Command { operation, deadline: original_deadline });
            Ok(())
        })
    }
    /// Queue one correlated closed outcome after root's independent actual launch admission.
    /// # Errors
    /// Rejects changed request/owner, occupied result slots, stale operation or an excessive original probe.
    /// Retained actual results can settle after connection loss or expiry; expired writes are suppressed.
    pub fn reply(
        &self,
        connection: EntryConnectionId,
        request: EntryRequestId,
        operation: EntryOperationId,
        original_deadline: Moment,
        outcome: EntryOutcome,
    ) -> Result<(), EntryTransportError> {
        self.commit(|state, now| {
            if !self.shared.server || state.owner.is_none() {
                return Err(EntryTransportError::InvalidPhase);
            }
            let route_index = state
                .requests
                .iter()
                .position(|route| {
                    route.as_ref().is_some_and(|route| {
                        route.connection == connection && route.request == request
                    })
                })
                .ok_or(EntryTransportError::Authentication)?;
            let route =
                state.requests[route_index].as_ref().ok_or(EntryTransportError::InvalidPhase)?;
            if operation.get() <= route.latest {
                return Err(EntryTransportError::InvalidOperation);
            }
            if original_deadline > route.until
                || (original_deadline > now
                    && original_deadline > schedule::probe(now, route.until)?)
            {
                return Err(EntryTransportError::InvalidDeadline);
            }
            if route.reply.is_some() {
                return Err(EntryTransportError::Busy);
            }
            if let EntryOutcome::Accepted(run) = outcome
                && Some(run.owner) != state.owner
            {
                return Err(EntryTransportError::Authentication);
            }
            let mut charged = false;
            if let Ok(index) = state.index(connection)
                && !state.connections[index].closing
                && original_deadline > now
                && route.until > now
            {
                let entry = &state.connections[index];
                if entry.writing || entry.reply.is_some() || operation.get() <= entry.latest {
                    return Err(EntryTransportError::Busy);
                }
                state.reserve(index, 1)?;
                state.connections[index].latest = operation.get();
                charged = true;
            }
            let route =
                state.requests[route_index].as_mut().ok_or(EntryTransportError::InvalidPhase)?;
            route.latest = operation.get();
            route.reply = Some(Reply {
                command: Command { operation, deadline: original_deadline },
                request,
                outcome,
                charged,
            });
            Ok(())
        })
    }
    /// Fence one connection immediately; the native worker proves actual retirement separately.
    pub fn close_connection(&self, connection: EntryConnectionId) {
        let mut state = lock(&self.shared.state);
        if let Ok(index) = state.index(connection) {
            state.connections[index].closing = true;
        }
        drop(state);
        self.shared.changed();
    }
    /// Fence endpoint acceptance and commands; cancellation never fabricates native settlement.
    pub fn close(&self) {
        lock(&self.shared.state).stopped = true;
        self.shared.changed();
    }
}
impl Drop for CoordinatorEntryControl {
    fn drop(&mut self) {
        self.close();
    }
}
fn validate(
    connection: &Connection,
    operation: EntryOperationId,
    deadline: Moment,
    now: Moment,
) -> Result<(), EntryTransportError> {
    if connection.closing || connection.peer.is_none() {
        return Err(EntryTransportError::Closed);
    }
    if operation.get() <= connection.latest {
        return Err(EntryTransportError::InvalidOperation);
    }
    let ancestor = connection.until.ok_or(EntryTransportError::InvalidPhase)?;
    if deadline <= now || deadline > schedule::probe(now, ancestor)? {
        return Err(EntryTransportError::InvalidDeadline);
    }
    Ok(())
}
