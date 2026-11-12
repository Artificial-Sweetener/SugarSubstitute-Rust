// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reopen only the durable candidate under installation mutation custody.
use super::*;
use sugarsubstitute_contracts::installation::Candidate;

impl InstallationSession {
    /// Resume the exact downloaded candidate recorded by an earlier preparation.
    /// Reuses this session's pinned configuration and exclusion, reopening protected candidate
    /// ancestry without guessing from directory order. Call `PreparedInstallation::prepare_update`
    /// on the result to reauthorize, complete missing files and verify every payload.
    /// A persisted trial requires native process reconciliation and cannot use this method.
    /// A fresh session supplies explicit first-setup choices; existing partial configuration
    /// must match those choices byte-for-byte when completion is retried.
    /// # Errors
    /// Preserves missing, unknown, foreign or inconsistent state. A bundle must retain its
    /// authenticated download; generation metadata alone never grants package authority.
    pub fn resume_update(
        self,
        bundle: Arc<VerifiedBundle>,
        context: &JobContext,
    ) -> io::Result<PreparedInstallation> {
        check(context)?;
        if bundle.download_identity().is_none() {
            return Err(invalid("Update archive custody is absent"));
        }
        let store = super::super::SelectionStore::open(self.directory(), context)
            .map_err(io::Error::other)?;
        let snapshot = store
            .load(context)
            .map_err(io::Error::other)?
            .ok_or_else(|| invalid("Installation selection is absent"))?;
        let generation = match snapshot.candidate() {
            Some(Candidate::Staging { generation } | Candidate::Prepared { generation }) => {
                generation
            }
            _ => return Err(invalid("No resumable update candidate is recorded")),
        };
        if generation.package.as_str() != bundle.digest() {
            return Err(invalid("Resume package differs from the recorded candidate"));
        }
        let path = self.admission.root.path().join("releases").join(generation.id.as_str());
        let (pinned, marker) = generation::open(&path, bundle.digest(), context)?;
        check(context)?;
        Ok(PreparedInstallation {
            session: self,
            generation: path,
            bundle,
            _generation: pinned,
            _generation_marker: marker,
        })
    }
}
