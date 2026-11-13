// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fetch current signed roles for an installer whose compiled-in metadata has expired (C4).
//!
//! Nothing fetched here is trusted. The installer passes the bytes to its import, which verifies
//! every role under the installation's trust exactly as it verifies compiled-in metadata, so a
//! hostile or broken network can make the refresh fail but never make it succeed falsely.
use super::*;

/// Most successor roots fetched in one refresh; a longer chain fails rather than looping.
const MAX_ROOTS: u32 = 64;

/// Current signed roles as served by the repository, not yet verified.
pub struct InstallerMetadata {
    /// Successor roots in ascending version order, each signed by its predecessor.
    pub roots: Vec<(u32, Vec<u8>)>,
    /// Timestamp role bytes.
    pub timestamp: Vec<u8>,
    /// Snapshot bytes named by the timestamp.
    pub snapshot: Vec<u8>,
    /// Targets bytes named by the snapshot.
    pub targets: Vec<u8>,
    /// Channel descriptor bytes for the installer's request.
    pub descriptor: Vec<u8>,
}

impl Repository {
    /// Fetch roots from `next_root` until the repository has none newer, then the timestamp,
    /// the snapshot and targets versions it names, and `request`'s channel descriptor.
    /// Run on an admitted job and retire this owner afterwards, as for discovery.
    /// # Errors
    /// Preserves transport, policy, bound and naming failures; any failure means the refresh is
    /// unavailable, and the caller offers the user the installer's included version instead.
    pub fn installer_metadata(
        &mut self,
        next_root: u32,
        request: &Request,
        context: &JobContext,
    ) -> Result<InstallerMetadata, Failure> {
        self.require_settled()?;
        let mut roots = Vec::new();
        let mut version = next_root;
        loop {
            match self.metadata(&format!("{version}.root.json"), MAX_METADATA_BYTES, context) {
                Ok(root) => roots.push((version, root)),
                Err(Failure::Http(update_http::Failure::Policy(
                    sugarsubstitute_update::http::Rejection::Status(404),
                ))) => break,
                Err(cause) => return Err(cause),
            }
            version = version.checked_add(1).ok_or(sugarsubstitute_update::Error::Bound)?;
            if version - next_root > MAX_ROOTS {
                return Err(sugarsubstitute_update::Error::Bound.into());
            }
        }
        let timestamp = self.metadata("timestamp.json", MAX_METADATA_BYTES, context)?;
        let version = sugarsubstitute_update::named_snapshot(&timestamp)?;
        let snapshot =
            self.metadata(&format!("{version}.snapshot.json"), MAX_METADATA_BYTES, context)?;
        let version = sugarsubstitute_update::named_targets(&snapshot)?;
        let targets =
            self.metadata(&format!("{version}.targets.json"), MAX_METADATA_BYTES, context)?;
        let descriptor = self.metadata(&request.target_name(), MAX_OFFER_BYTES, context)?;
        Ok(InstallerMetadata { roots, timestamp, snapshot, targets, descriptor })
    }
}
