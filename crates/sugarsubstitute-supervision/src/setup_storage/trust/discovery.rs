// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish authenticated discovery references before network traversal resumes.
use super::*;

impl TrustStore {
    /// Fund an explicit extraction workspace from this retained trust owner's original admission.
    pub(crate) fn workspace_cache(
        &self,
        anchor: &Path,
        context: &JobContext,
    ) -> io::Result<super::super::cache::RuntimeCache> {
        super::super::cache::RuntimeCache::open_retained(
            anchor,
            super::super::cache::CachePolicy::default(),
            &self.workspace,
            context,
        )
    }

    /// Open disposable storage on demand without making trust operations depend on cache health.
    /// Metadata uses this store's original admission; cancellation and native work use this job.
    pub(crate) fn runtime_cache(
        &self,
        context: &JobContext,
    ) -> io::Result<super::super::cache::RuntimeCache> {
        check(context)?;
        let mut cached =
            self.cache.lock().map_err(|_| io::Error::other("Cache scope owner is poisoned"))?;
        if let Some(cache) = &*cached {
            return Ok(cache.clone());
        }
        let cache = super::super::cache::RuntimeCache::open_retained(
            &self._installation_root.path,
            super::super::cache::CachePolicy::default(),
            &self.workspace,
            context,
        )?;
        *cached = Some(cache.clone());
        Ok(cache)
    }

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
