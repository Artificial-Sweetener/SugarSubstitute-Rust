// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select existing authority or first initialization while retaining installation mutation custody.
use super::*;
use crate::setup_storage::InstallationSession;
use sugarsubstitute_contracts::installation::InstallationRecord;

impl InstallationSession {
    /// Open this installation's trust, or initialize it from the authenticated distribution.
    /// The repository identity stays fixed across signing-key rotations. Existing checkpoints
    /// always supply authority; the distribution root is used only for first initialization.
    /// Run on a storage worker with at least `TRUST_RESERVATION_BYTES` reserved. This operation
    /// does not claim application-run custody or declare an installed generation healthy.
    /// # Errors
    /// Preserves damaged or foreign state without resetting trust or installation identity.
    /// Invalid bootstrap roots leave selection unchanged. Interruption between selection and
    /// trust publication resumes from the same revision-zero identity on the next attempt.
    pub fn update_trust(
        &self,
        repository: [u8; 32],
        trusted_root: &[u8],
        context: &JobContext,
    ) -> Result<TrustStore, TrustFailure> {
        check(context)?;
        let root = self.directory();
        if TrustStore::has_state(root)? {
            return TrustStore::open(root, repository, context);
        }
        let _work = context.memory().claim(WORK_BYTES).map_err(io::Error::other)?;
        let trust = RepositoryTrust::bootstrap(trusted_root)?;
        if SelectionStore::existing(root, context).map_err(io::Error::other)?.is_none() {
            let initial =
                InstallationRecord::empty(crate::setup_storage::identity::installation()?);
            SelectionStore::initialize(root, &initial, context).map_err(io::Error::other)?;
        }
        TrustStore::initialize_verified(root, repository, trust, context)
    }
}

#[cfg(test)]
mod tests;
