// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle exact retained root outcomes independently of connection or native reply timeliness.
use super::*;
impl Active {
    pub(super) fn commands(&mut self, core: &Core, now: Moment) -> Result<(), EntryTransportError> {
        for route_index in 0..MAX_ENTRY_CONNECTIONS {
            let original = {
                let state = lock(&self.work.shared.state);
                state.requests.get(route_index).and_then(|route| route.as_ref()).and_then(|route| {
                    route.reply.map(|reply| {
                        (route.connection, route.peer, route.request, route.until, reply)
                    })
                })
            };
            let Some((connection, peer, request, until, reply)) = original else { continue };
            if reply.request != request {
                return Err(EntryTransportError::Authentication);
            }
            // Root owns actual launch timing/custody. Cache settlement cannot be refused merely
            // because the original requester disconnected or its native reply budget expired.
            self.replay.as_mut().ok_or(EntryTransportError::InvalidPhase)?.settle(
                peer,
                request,
                reply.outcome,
            )?;
            let index = { lock(&self.work.shared.state).index(connection).ok() };
            let timely = index.is_some_and(|index| {
                self.connections[index].phase == Phase::AwaitRoot
                    && self.connections[index].pending.is_none()
            }) && now < until
                && now < reply.command.deadline;
            if timely {
                let index = index.ok_or(EntryTransportError::InvalidPhase)?;
                self.send_outcome(
                    core,
                    index,
                    reply.outcome,
                    Some(reply.command.operation),
                    reply.command.deadline,
                    now,
                )?;
            } else if reply.charged
                && let Some(index) = index
            {
                let mut state = lock(&self.work.shared.state);
                let slot = &mut state.connections[index];
                if slot.reserved != 0 {
                    slot.reserved -= 1;
                }
            }
            // No pending record can be evicted. Release routing only after exact cache settlement.
            lock(&self.work.shared.state).requests[route_index] = None;
        }
        Ok(())
    }
    pub(super) fn cached_reply(
        &mut self,
        core: &Core,
        index: usize,
        now: Moment,
    ) -> Result<(), EntryTransportError> {
        let (peer, header) = {
            let state = lock(&self.work.shared.state);
            (
                state.connections[index].peer.ok_or(EntryTransportError::InvalidPhase)?,
                state.connections[index].header.ok_or(EntryTransportError::InvalidPhase)?,
            )
        };
        if let Some((outcome, until)) = self
            .replay
            .as_ref()
            .ok_or(EntryTransportError::InvalidPhase)?
            .settled(peer, header.request)
        {
            if now >= until {
                self.close_connection(core, index, None);
                return Ok(());
            }
            self.send_outcome(core, index, outcome, None, schedule::probe(now, until)?, now)?;
        }
        Ok(())
    }
    pub(super) fn send_outcome(
        &mut self,
        core: &Core,
        index: usize,
        outcome: EntryOutcome,
        operation: Option<EntryOperationId>,
        deadline: Moment,
        _observed: Moment,
    ) -> Result<(), EntryTransportError> {
        let header = lock(&self.work.shared.state).connections[index]
            .header
            .ok_or(EntryTransportError::InvalidPhase)?;
        let payload = match outcome {
            EntryOutcome::Accepted(run) => protocol::Payload::Accepted(run),
            EntryOutcome::Rejected(reason) => protocol::Payload::Rejected(reason),
        };
        let length = protocol::encode(
            &mut self.connections[index]
                .write
                .as_mut()
                .ok_or(EntryTransportError::InvalidPhase)?
                .bytes,
            &header,
            payload,
        )?;
        self.write_frame(core, index, length, Phase::Retiring, deadline, operation)
    }
}
