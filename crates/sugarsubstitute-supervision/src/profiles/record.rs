// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve schema-one profile exclusion independently of logical profile labels.

use crate::ClaimError;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
};

const MAGIC: &[u8; 8] = b"SSPROFL1";
pub(super) const RECORD_BYTES: usize = 160;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum State {
    Held = 1,
    Released = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Record {
    pub(super) account: [u8; 32],
    pub(super) folder: [u8; 24],
    pub(super) run: RunKey,
    pub(super) state: State,
}

impl Record {
    pub(super) fn encode(self) -> [u8; RECORD_BYTES] {
        let mut bytes = [0; RECORD_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[10] = self.state as u8;
        bytes[16..48].copy_from_slice(&self.account);
        bytes[48..72].copy_from_slice(&self.folder);
        bytes[72..104].copy_from_slice(self.run.owner.namespace.bytes());
        bytes[104..120].copy_from_slice(self.run.owner.epoch.bytes());
        bytes[120..128].copy_from_slice(&self.run.instance.get().to_le_bytes());
        bytes[128..136].copy_from_slice(&self.run.generation.get().to_le_bytes());
        bytes
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, ClaimError> {
        if bytes.len() != RECORD_BYTES
            || &bytes[..8] != MAGIC
            || bytes[8..10] != [1, 0]
            || bytes[11..16].iter().chain(&bytes[136..]).any(|byte| *byte != 0)
        {
            return Err(ClaimError::invalid("profile_record_schema"));
        }
        let state = match bytes[10] {
            1 => State::Held,
            2 => State::Released,
            _ => return Err(ClaimError::invalid("profile_record_state")),
        };
        let number = |start| u64::from_le_bytes(std::array::from_fn(|index| bytes[start + index]));
        let run = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new(std::array::from_fn(|index| bytes[72 + index])),
                epoch: CoordinatorEpoch::new(std::array::from_fn(|index| bytes[104 + index]))
                    .map_err(|cause| ClaimError::new("profile_record_epoch", cause))?,
            },
            instance: InstanceId::new(number(120))
                .map_err(|cause| ClaimError::new("profile_record_instance", cause))?,
            generation: RunGeneration::new(number(128))
                .map_err(|cause| ClaimError::new("profile_record_generation", cause))?,
        };
        Ok(Self {
            account: std::array::from_fn(|index| bytes[16 + index]),
            folder: std::array::from_fn(|index| bytes[48 + index]),
            run,
            state,
        })
    }

    /// An unlocked inode does not prove a previous browser lifetime stopped.
    pub(super) fn admits_successor(self, next: Self) -> bool {
        self.state == State::Released && self.same_store(next)
    }

    /// Current publication may retry only within its actual held run/store.
    pub(super) fn permits_replacement(self, next: Self) -> bool {
        self.same_store(next) && (self.state == State::Released || self.run == next.run)
    }

    fn same_store(self, other: Self) -> bool {
        self.account == other.account && self.folder == other.folder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn held() -> Result<Record, Box<dyn std::error::Error>> {
        Ok(Record {
            account: [3; 32],
            folder: [4; 24],
            state: State::Held,
            run: RunKey {
                owner: CoordinatorKey {
                    namespace: NamespaceId::new([1; 32]),
                    epoch: CoordinatorEpoch::new([2; 16])?,
                },
                instance: InstanceId::new(7)?,
                generation: RunGeneration::new(9)?,
            },
        })
    }
    #[test]
    fn exact160_byte_record_roundtrips_and_rejects_all_invalid_envelopes() -> TestResult {
        let record = held()?;
        let bytes = record.encode();
        assert_eq!(bytes.len(), 160);
        assert_eq!(Record::decode(&bytes)?, record);
        for length in 0..160 {
            assert!(Record::decode(&bytes[..length]).is_err());
        }
        let mut oversized = bytes.to_vec();
        oversized.push(0);
        assert!(Record::decode(&oversized).is_err());
        for offset in [0, 8, 9, 10, 11, 15, 136, 159] {
            let mut invalid = bytes;
            invalid[offset] = 255;
            assert!(Record::decode(&invalid).is_err());
        }
        for (start, end) in [(104, 120), (120, 128), (128, 136)] {
            let mut invalid = bytes;
            invalid[start..end].fill(0);
            assert!(Record::decode(&invalid).is_err());
        }
        Ok(())
    }
    #[test]
    fn held_predecessor_never_admits_even_matching_run_and_store() -> TestResult {
        let held = held()?;
        assert!(!held.admits_successor(held));
        let mut next = held;
        next.run.generation = RunGeneration::new(10)?;
        assert!(!held.admits_successor(next));
        assert!(!held.permits_replacement(next));
        assert!(held.permits_replacement(Record { state: State::Released, ..held }));
        Ok(())
    }
    #[test]
    fn released_successor_requires_actual_account_and_folder_identity() -> TestResult {
        let held = held()?;
        let released = Record { state: State::Released, ..held };
        let mut next = held;
        next.run.generation = RunGeneration::new(10)?;
        assert!(released.admits_successor(next));
        for wrong in [Record { account: [5; 32], ..next }, Record { folder: [6; 24], ..next }] {
            assert!(!released.admits_successor(wrong));
            assert!(!released.permits_replacement(wrong));
        }
        Ok(())
    }
}
