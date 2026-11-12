// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate real capture lifetimes independently of incident annotation subjects.
use crate::{
    backend_custody::BackendCustodyKey,
    incident::IncidentSubject,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, IdentityError, RunKey},
};
use sha2::{Digest, Sha256};
/// Full authoritative lifetime admitted into the shared capture inventory.
/// Matching facts alone grant no handler, bootstrap, native pin or process authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CaptureTarget {
    /// One genuine desktop instance and run generation.
    Desktop(RunKey),
    /// One genuine independently enrolled backend custody helper session.
    BackendCustody(BackendCustodyKey),
}
impl CaptureTarget {
    /// Return the unchanged coordinator owner used by the original source inventory.
    pub const fn owner(self) -> CoordinatorKey {
        match self {
            Self::Desktop(run) => run.owner,
            Self::BackendCustody(key) => key.owner(),
        }
    }
    /// Project a desktop run only when the actual target is a desktop.
    pub const fn desktop(self) -> Option<RunKey> {
        match self {
            Self::Desktop(run) => Some(run),
            Self::BackendCustody(_) => None,
        }
    }
    /// Project a backend session only when the actual target is backend custody.
    pub const fn backend_custody(self) -> Option<BackendCustodyKey> {
        match self {
            Self::BackendCustody(key) => Some(key),
            Self::Desktop(_) => None,
        }
    }
    /// Derive the persisted existing incident subject from this exact capture target.
    /// Backend helpers use Supervisor with the first16 SHA256 bytes over the fixed
    /// domain, owner namespace, owner epoch and little-endian session. The ledger
    /// retains BackendCustody identity; no desktop run or new persisted subject exists.
    /// # Errors
    /// Preserves an invalid all-zero derived epoch rather than fabricating freshness.
    pub fn capture_subject(self) -> Result<IncidentSubject, IdentityError> {
        match self {
            Self::Desktop(run) => Ok(IncidentSubject::Desktop(run)),
            Self::BackendCustody(key) => {
                let mut hash = Sha256::new();
                hash.update(b"SugarSubstitute.backend-custody.capture.v1\0");
                hash.update(key.owner().namespace.bytes());
                hash.update(key.owner().epoch.bytes());
                hash.update(key.session().get().to_le_bytes());
                let digest = hash.finalize();
                let mut epoch = [0; 16];
                epoch.copy_from_slice(&digest[..16]);
                Ok(IncidentSubject::Supervisor(CoordinatorKey {
                    namespace: key.owner().namespace,
                    epoch: CoordinatorEpoch::new(epoch)?,
                }))
            }
        }
    }
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
