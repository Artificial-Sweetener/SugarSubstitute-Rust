// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve a settled trial under its original, continuously retained native run custody.
use super::*;

impl InstallationRun {
    /// Finish an owned attempt after the caller observes its actual process-family settlement.
    /// Preserve committed acceptance even if its worker completion was lost. An unresolved
    /// trial is rejected once; subsequent preparation remains intact on repeated finalization.
    /// This requires the original live token, not a lock reacquired after a crash.
    /// # Errors
    /// Rejects foreign custody, changed activation history, cancellation and native failures.
    pub fn finish_trial(
        &self,
        trial: &OwnedTrial,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        self.check_trial(trial)?;
        let accepted = trial.snapshot.accept(&trial.attempt)?;
        let rejected = trial.snapshot.reject(&trial.attempt)?;
        let store = &self.custody.store;
        store.protected(context, |protection| {
            let _writer = super::super::writer(&store.root, protection, context)?;
            let current = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if current == trial.snapshot {
                store.publish(protection, &rejected, context)?;
                return Ok(rejected);
            }
            if preparation_successor(&current, &accepted)
                || preparation_successor(&current, &rejected)
            {
                return Ok(current);
            }
            Err(SelectionFailure::Changed)
        })
    }
}

/// Ordinary preparation may advance while this run still owns the same resolved activation.
fn preparation_successor(current: &InstallationRecord, resolved: &InstallationRecord) -> bool {
    if current.revision() == resolved.revision() {
        return current == resolved;
    }
    current.installation() == resolved.installation()
        && current.revision() > resolved.revision()
        && current.accepted() == resolved.accepted()
        && current.retained() == resolved.retained()
        && current.rejected() == resolved.rejected()
        && !matches!(current.candidate(), Some(Candidate::Trial { .. }))
}

#[cfg(test)]
mod tests;
