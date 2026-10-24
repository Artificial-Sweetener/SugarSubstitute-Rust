// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish authenticated discovery references before network traversal resumes.
use super::*;

impl TrustStore {
    /// Bind preparation storage to the physical installation that owns this trust.
    pub(crate) fn verify_installation(
        &self,
        installation: &Path,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        check(context)?;
        if Directory::open(installation, false)?.identity()?
            != self._installation_root.identity()?
        {
            return Err(invalid("Update trust belongs to another installation").into());
        }
        Ok(())
    }

    /// Read the next contiguous root version under brief local exclusion.
    /// # Errors
    /// Rejects unavailable, damaged or foreign trust; never resets the root version.
    pub fn next_root_version(&self, context: &JobContext) -> Result<u32, TrustFailure> {
        self.protected(context, |protection| {
            let _writer = self.writer(protection)?;
            Ok(self.read(protection)?.next_root_version()?)
        })
    }

    /// Durably authenticate the snapshot reference before its network request.
    /// # Errors
    /// Preserves verification/publication failures without granting target authority.
    pub fn snapshot_version(
        &self,
        timestamp: &[u8],
        now: i64,
        context: &JobContext,
    ) -> Result<u32, TrustFailure> {
        self.transition(context, |trust| trust.snapshot_version(timestamp, now))
    }

    /// Durably authenticate the targets reference before its network request.
    /// # Errors
    /// Saves accepted parent floors even if child verification fails.
    pub fn targets_version(
        &self,
        timestamp: &[u8],
        snapshot: &[u8],
        now: i64,
        context: &JobContext,
    ) -> Result<u32, TrustFailure> {
        self.transition(context, |trust| trust.targets_version(timestamp, snapshot, now))
    }
}
