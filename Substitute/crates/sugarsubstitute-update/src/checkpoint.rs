// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authoritative trust checkpoint; never treat accepted keys or rollback floors as disposable cache.
use crate::{Error, MAX_CHECKPOINT_BYTES, MAX_METADATA_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Floor {
    version: u32,
    digest: [u8; 32],
}
impl Floor {
    /// Check the accepted version and exact signed bytes before advancing its floor.
    pub(super) fn advance(&mut self, version: u32, bytes: &[u8]) -> Result<(), Error> {
        self.require(version)?;
        let digest = Sha256::digest(bytes).into();
        if version == self.version && digest != self.digest {
            return Err(Error::Equivocation);
        }
        self.version = version;
        self.digest = digest;
        Ok(())
    }
    /// Reject version rollback without granting authenticity to the proposed version.
    pub(super) fn require(&self, version: u32) -> Result<(), Error> {
        if version == 0 || version < self.version { Err(Error::Rollback) } else { Ok(()) }
    }
    /// The uninitialized version has no previously authenticated byte identity.
    fn valid(&self) -> bool {
        self.version != 0 || self.digest == [0; 32]
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Checkpoint {
    schema_version: u32,
    pub(super) root: Vec<u8>,
    pub(super) timestamp: Floor,
    pub(super) snapshot: Floor,
    pub(super) targets: Floor,
    pub(super) minimum_snapshot: u32,
    pub(super) minimum_targets: u32,
    pub(super) time_floor: i64,
}
impl Checkpoint {
    /// Bootstrap only from a root delivered through an already trusted distribution channel.
    pub(crate) fn new(root: Vec<u8>) -> Self {
        Self {
            schema_version: 1,
            root,
            timestamp: Floor::default(),
            snapshot: Floor::default(),
            targets: Floor::default(),
            minimum_snapshot: 0,
            minimum_targets: 0,
            time_floor: 0,
        }
    }
    /// Decode an authenticated local trust store without resetting unknown or damaged state.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_CHECKPOINT_BYTES {
            return Err(Error::Bound);
        }
        let value: Self = serde_json::from_slice(bytes)?;
        if value.schema_version != 1
            || value.root.is_empty()
            || value.root.len() > MAX_METADATA_BYTES
            || value.time_floor < 0
            || (value.timestamp.version == 0 && value.time_floor != 0)
            || !value.timestamp.valid()
            || !value.snapshot.valid()
            || !value.targets.valid()
            || value.minimum_snapshot < value.snapshot.version
            || value.minimum_targets < value.targets.version
            || (value.timestamp.version == 0) != (value.minimum_snapshot == 0)
            || (value.snapshot.version == 0) != (value.minimum_targets == 0)
        {
            return Err(Error::State);
        }
        Ok(value)
    }
    /// Newest authenticated targets version and the SHA-256 of its exact signed bytes.
    pub(crate) fn targets_floor(&self) -> (u32, [u8; 32]) {
        (self.targets.version, self.targets.digest)
    }
    /// Serialize the current accepted root and all verified role floors for atomic publication.
    pub(crate) fn encode(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_CHECKPOINT_BYTES {
            return Err(Error::Bound);
        }
        Ok(bytes)
    }
}
