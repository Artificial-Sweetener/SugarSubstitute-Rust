// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the running probe's exact engine generation without selecting it or claiming ownership.
use super::*;
use crate::setup_storage::{InheritedUpdaterProbe, bundle::Payload};
use std::ffi::OsStr;
use sugarsubstitute_contracts::installation::Trial;

impl InheritedUpdaterProbe {
    /// Verify exclusive native custody and the actual running image's complete trial identity.
    /// The caller supplies its own current executable and repository trust from producing policy.
    /// Run on an admitted storage worker with the ordinary probe verification reservation.
    /// This is read-only verification; the creator must observe process settlement separately.
    /// # Errors
    /// Rejects foreign/shared handles, stale trials, changed payloads, cancellation and storage errors.
    pub fn verify(
        &self,
        trust: &TrustStore,
        executable: &Path,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        trust.verify_installation(&self.installation, context)?;
        trust.protected(context, |protection| {
            trust.verify_updater_preparation(&self.exclusion, protection, context)
        })?;
        trust.verify_updater_probe(executable, self.request.attempt(), context)?;
        check(context)?;
        Ok(())
    }
}

impl TrustStore {
    /// Validate an engine probe against its existing protected trial, release receipt and bytes.
    /// The native entry supplies its own `current_exe`; untrusted path input is not evidence of
    /// execution. The parent must authenticate the actual process result separately. This method
    /// does not grant health/activation authority or acquire application or engine preparation locks.
    /// Execute on an admitted worker with `RESERVATION_BYTES + TRUST_OPERATION_RESERVATION_BYTES`
    /// beside the retained trust store. The creator must retain its payload and trial custody.
    /// # Errors
    /// Rejects missing continuity, stale/foreign attempts, a different compiled engine release,
    /// changed/unprotected payloads, foreign executable identity, cancellation and storage errors.
    pub(in crate::setup_storage) fn verify_updater_probe(
        &self,
        executable: &Path,
        expected: &Trial,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        check(context)?;
        self.protected(context, |protection| {
            let _writer = self.writer(protection)?;
            let state = self.updater_locked(protection, SelectionRead::Existing, context)?;
            if !matches!(state.candidate(), Some(Candidate::Trial { attempt }) if attempt == expected)
            {
                return Err(invalid("Updater probe does not match the pending trial").into());
            }
            let receipt = self.updater_receipt(&expected.generation, protection)?;
            if receipt.sequence != sugarsubstitute_update::ENGINE_RELEASE {
                return Err(invalid("Running updater release differs from its authenticated receipt").into());
            }
            let path = self.updater_path(&expected.generation);
            protection::with(context, |payload_protection| {
            payload_protection.directory(&path, Mode::Existing, context)?;
            payload_protection.directory(&path.join("app"), Mode::Existing, context)?;
            let payload = Payload::open(&path, expected.generation.package.as_str(), Component::Updater, context)?;
            payload.require_protected(payload_protection)?;
            if executable.file_name() != Some(OsStr::new("sugarsubstitute-updater.exe")) {
                return Err(invalid("Updater probe has a different executable role"));
            }
            let actual = Directory::open(
                executable.parent().ok_or_else(|| invalid("Updater executable parent absent"))?,
                false,
            )?;
            let selected = Directory::open(&path.join("app"), false)?;
            if actual.identity()? != selected.identity()? {
                return Err(invalid("Updater probe runs outside the selected generation"));
            }
            check(context)?;
            Ok(())
            }).map_err(TrustFailure::from)
        })
    }
}

#[cfg(test)]
mod tests;
