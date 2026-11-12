// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep bounded immutable admission records across independently authenticated reconnects.
use super::{protocol::Header, types::*};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryOutcome, EntryRejection, EntryRequestId},
    execution::{MemoryClaim, MemoryLease, Moment},
};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Eq, PartialEq)]
struct PeerKey {
    pid: u32,
    birth: u64,
    request: EntryRequestId,
}
#[derive(Clone, Copy)]
struct Record {
    key: PeerKey,
    header: Header,
    received: Moment,
    until: Moment,
    length: usize,
    outcome: Option<EntryOutcome>,
}
struct Slot {
    body: Box<[u8]>,
    record: Option<Record>,
    _claim: MemoryClaim,
}
pub(super) enum Decision {
    New(usize),
    Pending,
    Cached { outcome: EntryOutcome, until: Moment },
    Rejected(EntryRejection),
}
pub(super) struct Replay {
    slots: Vec<Slot>,
    floor: u64,
}
impl Replay {
    pub(super) fn new(memory: &MemoryLease) -> Result<Self, EntryTransportError> {
        let mut slots = Vec::with_capacity(MAX_ENTRY_CONNECTIONS);
        for _ in 0..MAX_ENTRY_CONNECTIONS {
            let claim = memory.claim(MAX_ENTRY_FRAME_BYTES as u64).map_err(|cause| {
                EntryTransportError::native(crate::native::NativeObservationError::new(
                    "entry_replay_storage",
                    cause,
                ))
            })?;
            slots.push(Slot {
                body: vec![0; MAX_ENTRY_FRAME_BYTES].into_boxed_slice(),
                record: None,
                _claim: claim,
            });
        }
        Ok(Self { slots, floor: 0 })
    }
    // The worker verifies current kernel identity, epoch, nonce and original source
    // horizon before lookup. This cache cannot manufacture any of those facts.
    pub(super) fn consider(
        &mut self,
        peer: VerifiedEntryPeer,
        header: Header,
        body: &[u8],
        received: Moment,
        until: Moment,
    ) -> Result<Decision, EntryTransportError> {
        if header.request.get() != header.source.ticks() as u64
            || header.namespace != peer.namespace()
            || header.epoch.is_none()
            || body.len() > MAX_ENTRY_FRAME_BYTES
            || received >= until
        {
            return Err(EntryTransportError::Authentication);
        }
        let key = PeerKey { pid: peer.pid, birth: peer.birth, request: header.request };
        if let Some(slot) = self.slots.iter().find(|slot| slot.record.is_some_and(|r| r.key == key))
        {
            let Some(record) = slot.record else { return Err(EntryTransportError::InvalidPhase) };
            if !record.header.same_source(header)
                || record.header.epoch != header.epoch
                || slot.body[..record.length] != *body
            {
                return Ok(Decision::Rejected(EntryRejection::ChangedDuplicate));
            }
            if received >= record.until {
                return Ok(Decision::Rejected(EntryRejection::Deadline));
            }
            return Ok(match record.outcome {
                Some(outcome) => Decision::Cached { outcome, until: record.until },
                None => Decision::Pending,
            });
        }
        if header.request.get() <= self.floor {
            return Ok(Decision::Rejected(EntryRejection::StaleRequest));
        }
        let pending = self
            .slots
            .iter()
            .filter(|slot| slot.record.is_some_and(|r| r.outcome.is_none()))
            .count();
        let outcome = (pending >= MAX_ENTRY_CONNECTIONS - 1)
            .then_some(EntryOutcome::Rejected(EntryRejection::Capacity));
        let index = if let Some(index) = self.slots.iter().position(|slot| slot.record.is_none()) {
            index
        } else {
            let index = self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(index, slot)| {
                    slot.record
                        .filter(|r| r.outcome.is_some())
                        .map(|r| (index, r.key.request.get()))
                })
                .min_by_key(|(_, request)| *request)
                .map(|(index, _)| index)
                .ok_or(EntryTransportError::Capacity)?;
            if let Some(evicted) = self.slots[index].record {
                self.floor = self.floor.max(evicted.key.request.get());
            }
            index
        };
        let slot = &mut self.slots[index];
        slot.body[..body.len()].copy_from_slice(body);
        slot.record = Some(Record { key, header, received, until, length: body.len(), outcome });
        Ok(match outcome {
            Some(outcome) => Decision::Cached { outcome, until },
            None => Decision::New(index),
        })
    }
    pub(super) fn settle(
        &mut self,
        peer: VerifiedEntryPeer,
        request: EntryRequestId,
        outcome: EntryOutcome,
    ) -> Result<(), EntryTransportError> {
        let key = PeerKey { pid: peer.pid, birth: peer.birth, request };
        let record = self
            .slots
            .iter_mut()
            .find_map(|slot| slot.record.as_mut().filter(|r| r.key == key))
            .ok_or(EntryTransportError::InvalidPhase)?;
        if let Some(previous) = record.outcome {
            if previous != outcome {
                return Err(EntryTransportError::InvalidPhase);
            }
        } else {
            record.outcome = Some(outcome);
        }
        Ok(())
    }
    pub(super) fn settled(
        &self,
        peer: VerifiedEntryPeer,
        request: EntryRequestId,
    ) -> Option<(EntryOutcome, Moment)> {
        let key = PeerKey { pid: peer.pid, birth: peer.birth, request };
        self.slots.iter().find_map(|slot| {
            slot.record
                .filter(|record| record.key == key)
                .and_then(|record| record.outcome.map(|outcome| (outcome, record.until)))
        })
    }
    pub(super) fn budget(&self, index: usize) -> Result<(Moment, Moment), EntryTransportError> {
        self.slots
            .get(index)
            .and_then(|slot| slot.record)
            .map(|record| (record.received, record.until))
            .ok_or(EntryTransportError::InvalidPhase)
    }
}
