// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore accepted launch selection after trial ownership is lost, without retiring payloads.
use super::*;

impl InstallationRun {
    /// Resolve an interrupted trial whose ending nobody observed, under newly acquired
    /// installation-run exclusion. Original run and witness holders exclude this owner while they
    /// retain custody. A run that has admitted an application or trial cannot use this operation
    /// on its own work. The candidate returns to prepared without a verdict, except that the
    /// third abandoned trial of one package in a row counts as a strike (see
    /// `InstallationRecord::abandon_trial`).
    /// This changes selection only: it neither proves descendant settlement nor grants payload
    /// deletion, process termination, backend ownership or health authority. The accepted
    /// generation remains subject to ordinary verified launch and resource-claim admission.
    /// Repeated recovery preserves acceptance and subsequent preparation.
    /// # Errors
    /// Preserves corrupt state, cancellation and native publication failures. Execute on an
    /// admitted storage worker while retaining this run until publication has completed.
    pub(in crate::setup_storage) fn recover_abandoned_trial(
        &self,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        if self.issued {
            return Err(SelectionFailure::RunOwnership);
        }
        let store = &self.custody.store;
        store.protected(context, |protection| {
            let _writer = super::super::writer(&store.root, protection, context)?;
            let current = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
            let Some(Candidate::Trial { attempt }) = current.candidate() else {
                return Ok(current);
            };
            let recovered = current.abandon_trial(attempt, store.strike_clock().now()?)?;
            store.publish(protection, &recovered, context)?;
            tracing::info!(
                operation = "installation_abandoned_trial_recovered",
                installation = attempt.installation.as_str(),
                generation = attempt.generation.id.as_str(),
                revision = attempt.revision,
                prepared = matches!(recovered.candidate(), Some(Candidate::Prepared { .. })),
                "Resolved an abandoned trial while retaining interrupted payloads"
            );
            Ok(recovered)
        })
    }
}

impl SelectionStore {
    /// Load selection, recovering an abandoned trial only while exclusive run custody permits.
    /// Healthy applications and independent preparation require no run exclusion. Callers must
    /// still compare selection revisions before mutation or pin the selected record for launch.
    /// This grants no process-settlement or cleanup authority.
    pub(in crate::setup_storage) fn load_recovered(
        self: &Arc<Self>,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        let current = self.load(context)?.ok_or(SelectionFailure::Missing)?;
        if !matches!(current.candidate(), Some(Candidate::Trial { .. })) {
            return Ok(current);
        }
        InstallationRun::acquire(Arc::clone(self), context)?.recover_abandoned_trial(context)
    }
}

#[cfg(test)]
mod tests;
