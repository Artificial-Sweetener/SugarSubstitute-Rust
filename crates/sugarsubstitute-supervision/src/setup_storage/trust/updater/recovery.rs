// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Quarantine abandoned engine trials only after fresh native exclusion proves custody was released.
use super::*;

impl TrustStore {
    /// Recover an interrupted updater trial while preserving accepted and retained generations.
    /// Fresh updater exclusion must succeed; every retained duplicate excludes this operation.
    /// Execute on an admitted storage worker with ordinary trust-operation capacity. No application
    /// run or setup lease is acquired. This changes selection only and grants neither health,
    /// process termination nor payload cleanup authority. Missing continuity is never initialized.
    /// # Errors
    /// Rejects live custody, busy writers, invalid journals, cancellation and publication failure.
    pub fn recover_updater_trial(
        &self,
        context: &JobContext,
    ) -> Result<InstallationRecord, TrustFailure> {
        let _preparation = self.claim_updater_preparation(context)?;
        self.protected(context, |protection| {
            let _writer = self.writer(protection, context)?;
            let state = self.updater_locked(protection, SelectionRead::Existing, context)?;
            self.recover_updater_locked(state, protection, context)
        })
    }

    /// The caller retains freshly acquired updater exclusion and the trust writer throughout.
    /// Rejection needs the protected journal, not damaged candidate payloads or release receipts.
    pub(super) fn recover_updater_locked(
        &self,
        state: InstallationRecord,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<InstallationRecord, TrustFailure> {
        let Some(Candidate::Trial { attempt }) = state.candidate() else {
            return Ok(state);
        };
        let recovered = state
            .reject(attempt, crate::setup_storage::selection::unix_now().map_err(io::Error::other)?)
            .map_err(io::Error::other)?;
        self.write_updater(&recovered, protection, context)?;
        tracing::info!(
            operation = "updater_abandoned_trial_recovered",
            installation = attempt.installation.as_str(),
            generation = attempt.generation.id.as_str(),
            revision = attempt.revision,
            "Quarantined interrupted updater trial while retaining accepted generations"
        );
        Ok(recovered)
    }
}

#[cfg(test)]
mod tests;
