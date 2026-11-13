// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve legacy generations and atomically publish authenticated backend custody records.
use super::*;
use crate::coordinator_entry::EntryHolder;
pub(super) mod storage;
#[cfg(test)]
mod tests;
pub(crate) use storage::{
    CoordinatorPredecessor, check_predecessor, publish, publish_elected, retire_backend,
    retire_coordinator,
};
#[cfg(test)]
pub(crate) use storage::{
    permits_replacement, prepare_backend_release, prepare_coordinator_release,
};

const MAGIC: &[u8; 8] = b"SSCLAIM1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecordState {
    Held = 1,
    LaunchPending = 2,
    Owned = 3,
    Released = 4,
}
#[derive(Clone, Debug)]
pub(crate) struct Record {
    pub(crate) version: u16,
    pub(crate) custodian: u64,
    pub(crate) owner: CoordinatorKey,
    pub(crate) resource: u64,
    pub(crate) generation: u64,
    pub(crate) process: u64,
    pub(crate) digest: [u8; 32],
    pub(crate) state: RecordState,
    /// The coordinator process that published a coordinator record, kept through release, so
    /// a launch that cannot reach that coordinator can still ask whether it responds.
    pub(crate) holder: Option<EntryHolder>,
}
impl Record {
    pub(crate) fn coordinator(owner: CoordinatorKey) -> Self {
        Self {
            version: 1,
            custodian: 0,
            owner,
            resource: 0,
            generation: 0,
            process: 0,
            digest: *owner.namespace.bytes(),
            state: RecordState::Held,
            holder: None,
        }
    }
    /// Name the coordinator process publishing this coordinator record.
    pub(crate) fn held_by(mut self, holder: EntryHolder) -> Self {
        self.holder = Some(holder);
        self
    }
    pub(crate) fn backend(resource: ResourceKey, digest: [u8; 32]) -> Self {
        Self {
            version: 1,
            custodian: 0,
            owner: resource.owner,
            resource: resource.id.get(),
            generation: resource.generation.get(),
            process: 0,
            digest,
            state: RecordState::Held,
            holder: None,
        }
    }
    pub(crate) fn encode(&self) -> [u8; 128] {
        let mut bytes = [0; 128];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&self.version.to_le_bytes());
        bytes[10] = u8::from(self.resource != 0);
        bytes[11] = self.state as u8;
        bytes[12..44].copy_from_slice(self.owner.namespace.bytes());
        bytes[44..60].copy_from_slice(self.owner.epoch.bytes());
        bytes[60..68].copy_from_slice(&self.resource.to_le_bytes());
        // A coordinator record has no generation or process of its own; those slots name the
        // coordinator process that holds it.
        let (generation, process) = match (self.resource, self.holder) {
            (0, Some(holder)) => (holder.creation_time_100ns, u64::from(holder.process_id)),
            _ => (self.generation, self.process),
        };
        bytes[68..76].copy_from_slice(&generation.to_le_bytes());
        bytes[76..84].copy_from_slice(&process.to_le_bytes());
        bytes[84..116].copy_from_slice(&self.digest);
        bytes[116..124].copy_from_slice(&self.custodian.to_le_bytes());
        bytes
    }
    pub(crate) fn decode(bytes: &[u8; 128]) -> Result<Self, ClaimError> {
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if &bytes[..8] != MAGIC
            || !(1..=2).contains(&version)
            || bytes[124..].iter().any(|b| *b != 0)
        {
            return Err(ClaimError::invalid("claim_record_schema"));
        }
        let number = |start| u64::from_le_bytes(std::array::from_fn(|i| bytes[start + i]));
        let custodian = number(116);
        let resource = number(60);
        let (generation, process, holder) = if bytes[10] == 0 {
            (0, 0, decode_holder(number(76), number(68))?)
        } else {
            (number(68), number(76), None)
        };
        let state = match bytes[11] {
            1 => RecordState::Held,
            2 => RecordState::LaunchPending,
            3 => RecordState::Owned,
            4 => RecordState::Released,
            _ => return Err(ClaimError::invalid("claim_record_state")),
        };
        if (version == 1 && custodian != 0)
            || (version == 2 && custodian == 0)
            || bytes[10] > 1
            || (version == 2 && bytes[10] != 1)
            || (bytes[10] == 0 && (resource != 0 || generation != 0 || process != 0))
            || (bytes[10] == 0 && !matches!(state, RecordState::Held | RecordState::Released))
            || (bytes[10] == 1 && (resource == 0 || generation == 0))
            || (matches!(state, RecordState::Held | RecordState::Released) && process != 0)
            || (state == RecordState::Owned && process == 0)
        {
            return Err(ClaimError::invalid("claim_record_shape"));
        }
        let owner = CoordinatorKey {
            namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new(std::array::from_fn(
                |i| bytes[12 + i],
            )),
            epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new(
                std::array::from_fn(|i| bytes[44 + i]),
            )
            .map_err(|e| ClaimError::new("claim_record_epoch", e))?,
        };
        if bytes[10] == 0 && bytes[84..116] != owner.namespace.bytes()[..] {
            return Err(ClaimError::invalid("claim_record_namespace"));
        }
        Ok(Self {
            version,
            custodian,
            owner,
            resource,
            generation,
            process,
            state,
            digest: std::array::from_fn(|i| bytes[84 + i]),
            holder,
        })
    }
}
/// Both identity slots are zero for a record that names no holder, or both are set.
fn decode_holder(process: u64, birth: u64) -> Result<Option<EntryHolder>, ClaimError> {
    match (process, birth) {
        (0, 0) => Ok(None),
        (process, birth) if process != 0 && birth != 0 => Ok(Some(EntryHolder {
            process_id: u32::try_from(process)
                .map_err(|_| ClaimError::invalid("claim_record_holder"))?,
            creation_time_100ns: birth,
        })),
        _ => Err(ClaimError::invalid("claim_record_holder")),
    }
}
