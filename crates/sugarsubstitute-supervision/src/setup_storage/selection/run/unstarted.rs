// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact pre-publication custody for a creator that has not started a process.
use super::*;

/// Continuously held run custody established before this creator can publish its trial.
/// Only the native creator may restore it after permanently closing creation admission.
pub(crate) struct UnstartedTrial {
    custody: Arc<Custody>,
    prepared: InstallationRecord,
    trial: InstallationRecord,
    restored: InstallationRecord,
}
impl InstallationRun {
    /// Capture the exact prepared record before publication; a later error can have committed.
    pub(in crate::setup_storage) fn reserve_unstarted(
        &self,
        prepared: &InstallationRecord,
        context: &JobContext,
    ) -> Result<UnstartedTrial, SelectionFailure> {
        if self.issued {
            return Err(SelectionFailure::RunOwnership);
        }
        let trial = prepared.begin_trial()?;
        let Some(Candidate::Trial { attempt }) = trial.candidate() else {
            return Err(SelectionFailure::RunOwnership);
        };
        let restored = trial.cancel_unstarted_trial(attempt)?;
        if self.custody.store.load(context)?.as_ref() != Some(prepared) {
            return Err(SelectionFailure::Changed);
        }
        Ok(UnstartedTrial {
            custody: Arc::clone(&self.custody),
            prepared: prepared.clone(),
            trial,
            restored,
        })
    }
}
impl UnstartedTrial {
    /// Restore only this never-started attempt under its continuously held original run lock.
    /// Exact prepared/restored records make recovery retryable across post-publication failures.
    /// This grants no launch authority and cannot be reconstructed by reopening the installation.
    pub(in crate::setup_storage) fn restore(
        &self,
        installation: &crate::setup_storage::files::Directory,
        prepared: &mut InstallationRecord,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        let store = &self.custody.store;
        if installation.identity()? != store._installation.identity()? {
            return Err(SelectionFailure::RunOwnership);
        }
        if *prepared != self.prepared && *prepared != self.restored {
            return Err(SelectionFailure::Changed);
        }
        let restored = store.protected(context, |protection| {
            let _writer = super::super::writer(&store.root, protection, context)?;
            let current = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if *prepared == self.restored && current != self.restored {
                return Err(SelectionFailure::Changed);
            }
            if current == self.prepared || current == self.restored {
                return Ok(current);
            }
            if current != self.trial {
                return Err(SelectionFailure::Changed);
            }
            store.publish(protection, &self.restored, context)?;
            Ok(self.restored.clone())
        })?;
        *prepared = restored;
        Ok(())
    }
}
