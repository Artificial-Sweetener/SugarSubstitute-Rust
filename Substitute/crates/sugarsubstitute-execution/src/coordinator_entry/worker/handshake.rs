// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate actual kernel peers before closed source, epoch and nonce correlation.
use super::*;
use sugarsubstitute_contracts::lifecycle::CoordinatorKey;
#[derive(Clone, Copy)]
enum Received {
    Hello,
    Challenge,
    Launch,
    Outcome(EntryOutcome),
    Closing,
    Initializing,
}
impl Active {
    pub(super) fn source_until(
        &mut self,
        header: protocol::Header,
        received: Moment,
    ) -> Result<Moment, EntryTransportError> {
        if header.namespace != self.work.shared.facts.namespace()
            || header.request.get() != header.source.ticks() as u64
        {
            return Err(EntryTransportError::Authentication);
        }
        let sample = self.lease()?.sample_counter().map_err(EntryTransportError::Clock)?;
        let translated = crate::counter_deadline::decode(received, sample, header.source)
            .map_err(EntryTransportError::Clock)?;
        admission::validate_horizon(sample, header.source)?;
        // Remaining native duration is bounded first; central conversion already floors it.
        Ok(translated)
    }
    pub(super) fn received(
        &mut self,
        core: &Core,
        index: usize,
        length: usize,
        _iteration: Moment,
    ) -> Result<(), EntryTransportError> {
        if !(protocol::HEADER_BYTES..=MAX_ENTRY_FRAME_BYTES).contains(&length) {
            return Err(EntryTransportError::Malformed);
        }
        let peer = self.verified_peer(index, true)?;
        let (header, payload) = {
            let buffer =
                self.connections[index].read.as_ref().ok_or(EntryTransportError::InvalidPhase)?;
            let frame = protocol::decode(&buffer.bytes[..length])?;
            let payload = match frame.payload {
                protocol::DecodedPayload::Hello => Received::Hello,
                protocol::DecodedPayload::Challenge => Received::Challenge,
                protocol::DecodedPayload::Launch(_) => Received::Launch,
                protocol::DecodedPayload::Accepted(run) => {
                    Received::Outcome(EntryOutcome::Accepted(run))
                }
                protocol::DecodedPayload::Rejected(reason) => {
                    Received::Outcome(EntryOutcome::Rejected(reason))
                }
                protocol::DecodedPayload::Closing => Received::Closing,
                protocol::DecodedPayload::Initializing => Received::Initializing,
            };
            (frame.header, payload)
        };
        // Capture destination before actual native sampling; use the sole counter owner.
        let received = core.clock.now();
        let translated = self.source_until(header, received)?;
        match (self.work.shared.server, self.connections[index].phase, payload) {
            (true, Phase::Hello, Received::Hello) => {
                self.server_hello(core, index, header, translated, received)
            }
            (true, Phase::Launch, Received::Launch) => {
                self.server_launch(core, index, (peer, header), length, translated, received)
            }
            (false, Phase::Challenge, Received::Initializing) => {
                let original =
                    self.client.as_ref().ok_or(EntryTransportError::InvalidPhase)?.header;
                if !original.same_source(header) || header.epoch.is_some() {
                    return Err(EntryTransportError::Authentication);
                }
                self.emit(
                    core,
                    Some(index),
                    EntryTransportObservation::DiscoveryVerified {
                        connection: peer.connection(),
                        phase: EntryDiscoveryPhase::Initializing,
                        peer,
                    },
                )?;
                self.connections[index].retry_at =
                    Some(schedule::probe(received, self.work.shared.original_preparation)?);
                self.close_connection(core, index, None);
                Ok(())
            }
            (false, Phase::Challenge, Received::Challenge) => {
                self.client_challenge(core, index, peer, header, received)
            }
            (false, Phase::Outcome, Received::Outcome(outcome)) => {
                let expected = lock(&self.work.shared.state).connections[index]
                    .header
                    .ok_or(EntryTransportError::InvalidPhase)?;
                if !expected.same_source(header)
                    || expected.epoch != header.epoch
                    || expected.nonce != header.nonce
                {
                    return Err(EntryTransportError::Authentication);
                }
                if let EntryOutcome::Accepted(run) = outcome
                    && (run.owner.namespace != header.namespace
                        || Some(run.owner.epoch) != header.epoch)
                {
                    return Err(EntryTransportError::Authentication);
                }
                self.emit(
                    core,
                    Some(index),
                    EntryTransportObservation::OutcomeReceived {
                        connection: peer.connection(),
                        request: header.request,
                        outcome,
                    },
                )?;
                self.close_connection(core, index, None);
                Ok(())
            }
            (_, _, Received::Closing) => {
                if !self.work.shared.server && self.connections[index].phase == Phase::Challenge {
                    let original =
                        self.client.as_ref().ok_or(EntryTransportError::InvalidPhase)?.header;
                    if !original.same_source(header)
                        || original.epoch.is_some_and(|epoch| Some(epoch) != header.epoch)
                    {
                        return Err(EntryTransportError::Authentication);
                    }
                    if self.work.input.as_ref().is_some_and(|input| input.recover_unsubmitted) {
                        self.return_unsubmitted = true;
                        self.connections[index].retry_at =
                            Some(schedule::probe(received, self.work.shared.original_preparation)?);
                    }
                }
                self.close_connection(core, index, None);
                Ok(())
            }
            _ => Err(EntryTransportError::InvalidPhase),
        }
    }
    fn server_hello(
        &mut self,
        core: &Core,
        index: usize,
        original: protocol::Header,
        translated: Moment,
        now: Moment,
    ) -> Result<(), EntryTransportError> {
        let owner = lock(&self.work.shared.state).owner;
        let until = if owner.is_some() {
            translated
        } else {
            translated.min(self.work.shared.original_preparation)
        };
        if until <= now {
            return Err(EntryTransportError::InvalidDeadline);
        }
        let mut header = original;
        let (payload, next) = if let Some(owner) = owner {
            if original.epoch.is_some_and(|candidate| candidate != owner.epoch) {
                return Err(EntryTransportError::Authentication);
            }
            header.epoch = Some(owner.epoch);
            header.nonce = self.lease()?.nonce()?;
            (protocol::Payload::Challenge, Phase::Launch)
        } else {
            header.epoch = None;
            header.nonce = [0; 32];
            (protocol::Payload::Initializing, Phase::Retiring)
        };
        {
            let mut state = lock(&self.work.shared.state);
            state.connections[index].header = Some(header);
            state.connections[index].until = Some(until);
        }
        let length = protocol::encode(
            &mut self.connections[index]
                .write
                .as_mut()
                .ok_or(EntryTransportError::InvalidPhase)?
                .bytes,
            &header,
            payload,
        )?;
        self.write_frame(core, index, length, next, schedule::probe(now, until)?, None)
    }
    fn client_challenge(
        &mut self,
        core: &Core,
        index: usize,
        peer: VerifiedEntryPeer,
        header: protocol::Header,
        now: Moment,
    ) -> Result<(), EntryTransportError> {
        let original = self.client.as_ref().ok_or(EntryTransportError::InvalidPhase)?.header;
        if !original.same_source(header)
            || original.epoch.is_some_and(|epoch| Some(epoch) != header.epoch)
        {
            return Err(EntryTransportError::Authentication);
        }
        let epoch = header.epoch.ok_or(EntryTransportError::Authentication)?;
        let owner = CoordinatorKey { namespace: header.namespace, epoch };
        self.emit(
            core,
            Some(index),
            EntryTransportObservation::DiscoveryVerified {
                connection: peer.connection(),
                phase: EntryDiscoveryPhase::Serving(owner),
                peer,
            },
        )?;
        lock(&self.work.shared.state).connections[index].header = Some(header);
        let buffer =
            self.connections[index].write.as_mut().ok_or(EntryTransportError::InvalidPhase)?;
        protocol::bind_staged_launch(
            &mut buffer.bytes,
            self.client.as_ref().ok_or(EntryTransportError::InvalidPhase)?.length,
            &header,
        )?;
        self.write_frame(
            core,
            index,
            self.client.as_ref().ok_or(EntryTransportError::InvalidPhase)?.length,
            Phase::Outcome,
            schedule::probe(now, self.work.shared.original_preparation)?,
            None,
        )
    }
    fn server_launch(
        &mut self,
        core: &Core,
        index: usize,
        authenticated: (VerifiedEntryPeer, protocol::Header),
        length: usize,
        translated: Moment,
        now: Moment,
    ) -> Result<(), EntryTransportError> {
        let (peer, header) = authenticated;
        let (expected, latched, owner) = {
            let state = lock(&self.work.shared.state);
            (
                state.connections[index].header.ok_or(EntryTransportError::InvalidPhase)?,
                state.connections[index].until.ok_or(EntryTransportError::InvalidPhase)?,
                state.owner,
            )
        };
        if !expected.same_source(header)
            || expected.epoch != header.epoch
            || expected.nonce != header.nonce
            || owner.is_none_or(|owner| Some(owner.epoch) != header.epoch)
        {
            return Err(EntryTransportError::Authentication);
        }
        let until = translated.min(latched);
        if now >= until {
            return Err(EntryTransportError::InvalidDeadline);
        }
        let buffer =
            self.connections[index].read.as_ref().ok_or(EntryTransportError::InvalidPhase)?;
        let decision = self.replay.as_mut().ok_or(EntryTransportError::InvalidPhase)?.consider(
            peer,
            header,
            &buffer.bytes[protocol::HEADER_BYTES..length],
            now,
            until,
        )?;
        lock(&self.work.shared.state).connections[index].until = Some(until);
        match decision {
            replay::Decision::New(record) => {
                let (received, until) = self
                    .replay
                    .as_ref()
                    .ok_or(EntryTransportError::InvalidPhase)?
                    .budget(record)?;
                let buffer =
                    self.connections[index].read.take().ok_or(EntryTransportError::InvalidPhase)?;
                let request = AuthenticatedEntryLaunch {
                    frame: buffers::ReceivedEntryFrame {
                        buffer: Some(buffer),
                        length,
                        index,
                        shared: self.work.shared.clone(),
                    },
                    request: header.request,
                    received,
                    until,
                    source: header.source,
                    peer,
                };
                {
                    let mut state = lock(&self.work.shared.state);
                    state.connections[index].loan = true;
                    state.requests[record] = Some(RequestRoute {
                        connection: peer.connection(),
                        peer,
                        request: header.request,
                        until,
                        latest: 0,
                        reply: None,
                    });
                }
                self.connections[index].phase = Phase::AwaitRoot;
                self.emit(
                    core,
                    Some(index),
                    EntryTransportObservation::RequestAuthenticated {
                        connection: peer.connection(),
                        request,
                    },
                )?;
                self.report.authenticated_launches += 1;
                Ok(())
            }
            replay::Decision::Pending => {
                self.connections[index].phase = Phase::AwaitRoot;
                Ok(())
            }
            replay::Decision::Cached { outcome, until } => {
                lock(&self.work.shared.state).connections[index].until = Some(until);
                self.send_outcome(core, index, outcome, None, schedule::probe(now, until)?, now)
            }
            replay::Decision::Rejected(reason) => self.send_outcome(
                core,
                index,
                EntryOutcome::Rejected(reason),
                None,
                schedule::probe(now, until)?,
                now,
            ),
        }
    }
    pub(super) fn verified_peer(
        &mut self,
        index: usize,
        message: bool,
    ) -> Result<VerifiedEntryPeer, EntryTransportError> {
        let kernel = self.lease()?.verify(index, message)?;
        if kernel.facts != self.work.shared.facts || kernel.pid == 0 || kernel.birth == 0 {
            return Err(EntryTransportError::Authentication);
        }
        let state = lock(&self.work.shared.state);
        let connection = state.connections[index].id.ok_or(EntryTransportError::InvalidPhase)?;
        let peer = VerifiedEntryPeer {
            connection,
            facts: kernel.facts,
            pid: kernel.pid,
            birth: kernel.birth,
        };
        if state.connections[index].peer.is_some_and(|previous| previous != peer) {
            return Err(EntryTransportError::Authentication);
        }
        Ok(peer)
    }
}
