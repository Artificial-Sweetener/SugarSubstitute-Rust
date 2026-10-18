// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep fixed observation credits, passive loans and closed commands under one session lock.
use super::*;
use std::collections::VecDeque;
use sugarsubstitute_contracts::{
    coordinator_entry::*,
    execution::{JobId, MemoryClaim},
    lifecycle::CoordinatorKey,
};

#[derive(Clone, Copy)]
pub(super) struct Command {
    pub(super) operation: EntryOperationId,
    pub(super) deadline: Moment,
}
#[derive(Clone, Copy)]
pub(super) struct Reply {
    pub(super) command: Command,
    pub(super) request: EntryRequestId,
    pub(super) outcome: EntryOutcome,
    pub(super) charged: bool,
}
pub(super) struct RequestRoute {
    pub(super) connection: EntryConnectionId,
    pub(super) peer: VerifiedEntryPeer,
    pub(super) request: EntryRequestId,
    pub(super) until: Moment,
    pub(super) latest: u64,
    pub(super) reply: Option<Reply>,
}
pub(super) struct Connection {
    pub(super) id: Option<EntryConnectionId>,
    pub(super) peer: Option<VerifiedEntryPeer>,
    pub(super) header: Option<protocol::Header>,
    pub(super) until: Option<Moment>,
    pub(super) latest: u64,
    pub(super) read: Option<Command>,
    pub(super) reply: Option<Reply>,
    pub(super) closing: bool,
    pub(super) reading: bool,
    pub(super) writing: bool,
    pub(super) loan: bool,
    pub(super) returned: Option<buffers::Buffer>,
    pub(super) observed: usize,
    pub(super) reserved: usize,
}
impl Connection {
    pub(super) fn new() -> Self {
        Self {
            id: None,
            peer: None,
            header: None,
            until: None,
            latest: 0,
            read: None,
            reply: None,
            closing: false,
            reading: false,
            writing: false,
            loan: false,
            returned: None,
            observed: 0,
            reserved: 0,
        }
    }
}
pub(super) struct Envelope {
    pub(super) slot: Option<usize>,
    pub(super) observation: EntryTransportObservation,
}
pub(super) struct State {
    pub(super) unsubmitted: Option<UnsubmittedEntryRequest>,
    pub(super) prepared: bool,
    pub(super) stopped: bool,
    pub(super) owner: Option<CoordinatorKey>,
    pub(super) activation: Option<CoordinatorKey>,
    pub(super) connections: Vec<Connection>,
    pub(super) observations: VecDeque<Envelope>,
    pub(super) metadata_reserved: usize,
    pub(super) next_connection: u64,
    pub(super) requests: Vec<Option<RequestRoute>>,
}
impl State {
    pub(super) fn new(server: bool) -> Self {
        Self {
            unsubmitted: None,
            prepared: false,
            stopped: false,
            owner: None,
            activation: None,
            connections: Vec::new(),
            observations: VecDeque::new(),
            metadata_reserved: if server { 2 } else { 1 },
            next_connection: 0,
            requests: Vec::new(),
        }
    }
    pub(super) fn allocate(&mut self, count: usize) {
        self.connections = (0..count).map(|_| Connection::new()).collect();
        self.observations =
            VecDeque::with_capacity(if count == 1 { 8 } else { MAX_ENTRY_OBSERVATIONS });
        if count == MAX_ENTRY_CONNECTIONS {
            self.requests = (0..MAX_ENTRY_CONNECTIONS).map(|_| None).collect();
        }
    }
    pub(super) fn reserve(
        &mut self,
        index: usize,
        additional: usize,
    ) -> Result<(), EntryTransportError> {
        let occupied = self.observations.len()
            + self.metadata_reserved
            + self.connections.iter().map(|connection| connection.reserved).sum::<usize>();
        let limit = if self.connections.len() == 1 { 8 } else { MAX_ENTRY_OBSERVATIONS };
        let connection =
            self.connections.get_mut(index).ok_or(EntryTransportError::InvalidOperation)?;
        if occupied + additional > limit
            || connection.observed + connection.reserved + additional > 8
        {
            return Err(EntryTransportError::Busy);
        }
        connection.reserved += additional;
        Ok(())
    }
    pub(super) fn emit(
        &mut self,
        index: Option<usize>,
        observation: EntryTransportObservation,
    ) -> Result<(), EntryTransportError> {
        if let Some(index) = index {
            let connection =
                self.connections.get_mut(index).ok_or(EntryTransportError::InvalidOperation)?;
            if connection.reserved == 0 {
                return Err(EntryTransportError::Capacity);
            }
            connection.reserved -= 1;
            connection.observed += 1;
        } else {
            if self.metadata_reserved == 0 {
                return Err(EntryTransportError::Capacity);
            }
            self.metadata_reserved -= 1;
        }
        self.observations.push_back(Envelope { slot: index, observation });
        Ok(())
    }
    pub(super) fn index(&self, id: EntryConnectionId) -> Result<usize, EntryTransportError> {
        self.connections
            .iter()
            .position(|connection| connection.id == Some(id))
            .ok_or(EntryTransportError::Closed)
    }
}
pub(super) struct Shared {
    pub(super) server: bool,
    pub(super) facts: EntryNativeIdentityFacts,
    pub(super) original_preparation: Moment,
    pub(super) state: Mutex<State>,
    pub(super) metadata: Mutex<Option<MemoryClaim>>,
    pub(super) core: Weak<Core>,
    pub(super) job_id: JobId,
}
impl Shared {
    pub(super) fn changed(&self) {
        if let Some(core) = self.core.upgrade() {
            core.changed();
        }
    }
}
