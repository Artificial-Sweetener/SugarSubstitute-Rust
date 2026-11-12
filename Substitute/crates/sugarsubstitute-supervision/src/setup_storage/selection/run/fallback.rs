// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run the retained release when the accepted release's files are damaged, and repair it later.
use super::*;
use sugarsubstitute_contracts::installation::Generation;

impl InstallationRun {
    /// Make the retained generation accepted again because the accepted `failed` generation's
    /// engine is missing, mismatched, or never took over. `failed` is recorded for repair, so the
    /// next update check downloads and verifies the same release again; it gains no strike and
    /// no rollback because the release itself did nothing wrong. A repeat after this was already
    /// published returns the current record unchanged.
    /// Run under this live run's exclusion, before starting the retained generation.
    /// # Errors
    /// `Record` when the current record cannot fall back from `failed` (no retained generation,
    /// a live trial, or another accepted generation), plus lock, cancellation and storage
    /// failures.
    pub fn fall_back_to_retained(
        &self,
        failed: &Generation,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        let store = &self.custody.store;
        settlement::through_contention(context, || {
            store.protected(context, |protection| {
                let _writer = super::super::writer(&store.root, protection, context)?;
                let current = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
                if current.repair() == Some(failed) && current.accepted() != Some(failed) {
                    return Ok(current);
                }
                let fallen = current.fall_back_to_retained(failed)?;
                store.publish(protection, &fallen, context)?;
                tracing::warn!(
                    operation = "installation_fell_back_to_retained",
                    installation = fallen.installation().as_str(),
                    failed = failed.id.as_str(),
                    revision = fallen.revision(),
                    "Started the previous release because the accepted release's files are damaged"
                );
                Ok(fallen)
            })
        })
    }
}

#[cfg(test)]
mod tests;
