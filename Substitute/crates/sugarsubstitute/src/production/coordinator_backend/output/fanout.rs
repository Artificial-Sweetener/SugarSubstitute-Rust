// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one genuine native loan until every subscribed Parent consumes its own copy.
use super::*;
use sugarsubstitute_contracts::transcript::{OutputGap, OutputStream, StreamSequence};

/// Cumulative actual bytes include native loss; sequence remains the producer's original value.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::production::coordinator_backend) struct Position {
    pub(in crate::production::coordinator_backend) sequence: u64,
    pub(in crate::production::coordinator_backend) bytes: u64,
    pub(in crate::production::coordinator_backend) ended: bool,
}
/// Matching receipt minted only after copying this actual source into the Parent's funded slot.
/// It is not a native completion, lease grant or process-retirement proof.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Receipt {
    pub(in crate::production::coordinator_backend) lease: LeaseKey,
    pub(in crate::production::coordinator_backend) serial: u64,
    pub(in crate::production::coordinator_backend) stream: OutputStream,
    pub(in crate::production::coordinator_backend) position: Position,
}
/// Borrowed delivery metadata; the resource owner retains the noncloneable source guard.
pub(crate) struct Delivery {
    pub(crate) receipt: Receipt,
    pub(crate) stream: OutputStream,
    pub(crate) sequence: StreamSequence,
    pub(crate) gap: Option<OutputGap>,
    pub(crate) end: bool,
    pub(crate) count: usize,
}
#[derive(Clone, Copy)]
struct End {
    serial: u64,
    position: Position,
}
/// One existing output guard plus constant-size historical byte/EOF facts; no second byte queue.
#[derive(Default)]
pub(in crate::production::coordinator_backend) struct ResourceOutput {
    pub(in crate::production::coordinator_backend) current: Option<AdmittedOutputBatch>,
    pub(in crate::production::coordinator_backend) serial: u64,
    positions: [Position; 2],
    ends: [Option<End>; 2],
}
impl ResourceOutput {
    /// Adopt before any fallible inspection so malformed native output keeps its original funding.
    pub(in crate::production::coordinator_backend) fn adopt(
        &mut self,
        key: ResourceKey,
        guard: AdmittedOutputBatch,
    ) -> Result<()> {
        if self.current.is_some() {
            return Err(LifecycleRejection::WrongPhase.into());
        }
        self.current = Some(guard);
        let batch = self
            .current
            .as_ref()
            .and_then(AdmittedOutputBatch::batch)
            .ok_or(LifecycleRejection::WrongPhase)?;
        if batch.resource() != key {
            return Err(LifecycleRejection::StaleIdentity.into());
        }
        let index = batch.stream().index();
        let previous = self.positions[index];
        let expected = previous
            .sequence
            .checked_add(1)
            .and_then(|value| value.checked_add(batch.gap().map_or(0, OutputGap::skipped_batches)))
            .ok_or(LifecycleRejection::IdentityExhausted)?;
        if previous.ended || expected != batch.sequence().get() {
            return Err(LifecycleRejection::WrongPhase.into());
        }
        let position = Position {
            sequence: expected,
            bytes: previous
                .bytes
                .checked_add(batch.gap().map_or(0, OutputGap::dropped_bytes))
                .and_then(|value| value.checked_add(batch.bytes().len() as u64))
                .ok_or(LifecycleRejection::IdentityExhausted)?,
            ended: batch.end(),
        };
        let serial = self.serial.checked_add(1).ok_or(LifecycleRejection::IdentityExhausted)?;
        self.positions[index] = position;
        self.serial = serial;
        if position.ended {
            self.ends[index] = Some(End { serial, position });
        }
        Ok(())
    }
    /// Per-subscriber readiness never mistakes another Parent's native loan for unavailability.
    pub(in crate::production::coordinator_backend) fn available(
        &self,
        client: Option<&super::subscribers::Subscriber>,
    ) -> bool {
        if client.is_some_and(|client| client.pending.is_some()) {
            return false;
        }
        if self
            .ends
            .iter()
            .enumerate()
            .any(|(i, end)| end.is_some() && client.is_none_or(|client| !client.positions[i].ended))
        {
            return true;
        }
        self.current.as_ref().and_then(AdmittedOutputBatch::batch).is_some_and(|batch| {
            client.is_none_or(|client| {
                client.positions[batch.stream().index()].sequence < batch.sequence().get()
            })
        })
    }
    /// Replay an actual historical EOF for a late join, or offer the current original batch.
    /// Prefix gaps count exact bytes not seen by this subscriber, including original native loss.
    pub(in crate::production::coordinator_backend) fn copy(
        &self,
        client: &mut super::subscribers::Subscriber,
        destination: &mut [u8],
    ) -> Result<Option<Delivery>> {
        if client.pending.is_some() {
            return Err(LifecycleRejection::WrongPhase.into());
        }
        for (index, end) in self.ends.iter().enumerate() {
            if let Some(end) = end
                && !client.positions[index].ended
                && self.current.as_ref().and_then(AdmittedOutputBatch::batch).is_none_or(|batch| {
                    batch.stream().index() != index
                        || batch.sequence().get() != end.position.sequence
                })
            {
                let stream = if index == 0 { OutputStream::Stdout } else { OutputStream::Stderr };
                return Self::deliver(client, destination, end.serial, stream, end.position, &[]);
            }
        }
        let Some(batch) = self.current.as_ref().and_then(AdmittedOutputBatch::batch) else {
            return Ok(None);
        };
        if client.positions[batch.stream().index()].sequence >= batch.sequence().get() {
            return Ok(None);
        }
        Self::deliver(
            client,
            destination,
            self.serial,
            batch.stream(),
            self.positions[batch.stream().index()],
            batch.bytes(),
        )
    }
    fn deliver(
        client: &mut super::subscribers::Subscriber,
        destination: &mut [u8],
        serial: u64,
        stream: OutputStream,
        position: Position,
        bytes: &[u8],
    ) -> Result<Option<Delivery>> {
        let previous = client.positions[stream.index()];
        if previous.ended || destination.len() < bytes.len() {
            return Err(LifecycleRejection::WrongPhase.into());
        }
        let skipped = position
            .sequence
            .checked_sub(previous.sequence)
            .and_then(|value| value.checked_sub(1))
            .ok_or(LifecycleRejection::StaleIdentity)?;
        let missing = position
            .bytes
            .checked_sub(bytes.len() as u64)
            .and_then(|value| value.checked_sub(previous.bytes))
            .ok_or(LifecycleRejection::StaleIdentity)?;
        let gap = if missing == 0 {
            if skipped != 0 {
                return Err(LifecycleRejection::WrongPhase.into());
            }
            None
        } else {
            Some(OutputGap::new(missing, skipped)?)
        };
        let sequence = StreamSequence::new(position.sequence)?;
        let receipt = Receipt { lease: client.lease, serial, stream, position };
        destination[..bytes.len()].copy_from_slice(bytes);
        client.pending = Some(Receipt {
            lease: receipt.lease,
            serial: receipt.serial,
            stream: receipt.stream,
            position: receipt.position,
        });
        Ok(Some(Delivery {
            receipt,
            stream,
            sequence,
            gap,
            end: position.ended,
            count: bytes.len(),
        }))
    }
    /// The source loan returns only after all registered consumers ACK, or actually release.
    pub(in crate::production::coordinator_backend) fn release_consumed(
        &mut self,
        key: ResourceKey,
        clients: &super::Subscribers,
    ) {
        if self.current.is_some() && clients.consumed(key, self.serial) {
            self.current = None;
        }
    }
}
