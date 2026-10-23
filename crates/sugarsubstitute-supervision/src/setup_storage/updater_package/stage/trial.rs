// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain engine preparation and exact publication identity before any native creation admission.
use super::*;
use sugarsubstitute_contracts::installation::{Candidate, InstallationRecord, Trial};
mod transfer;
pub use transfer::{RejectedUpdaterProbe, UpdaterProbeTransfer};

/// Original pinned engine, authenticated source and exclusive preparation custody for one attempt.
/// No process has been admitted by this owner. Publication records a pending trial, never health.
/// Call `restore` before retiring an unused owner; dropping it preserves the durable journal.
/// Storage operations require admitted trust-operation capacity beside the retained preparation.
pub struct UnstartedUpdaterTrial {
    prepared: PreparedUpdater,
    snapshot: InstallationRecord,
    attempt: Trial,
    closed: bool,
}

impl PreparedUpdater {
    /// Reserve the exact prepared journal while retaining its original OS lock and pinned bytes.
    /// This is an inert reservation: no selection is changed and no process may be created.
    /// Call on an admitted storage worker with trust-operation capacity.
    /// # Errors
    /// Rejects missing probe support, cancellation, changed preparation, invalid protection
    /// and revision exhaustion without publishing a trial.
    pub fn reserve_trial(
        self,
        context: &JobContext,
    ) -> Result<UnstartedUpdaterTrial, TrustFailure> {
        self.payload.compatibility.require_updater_probe()?;
        let snapshot = self.source.trust.reserve_updater_trial(&self.generation, context)?;
        let trial = snapshot.begin_trial().map_err(io::Error::other)?;
        let Some(Candidate::Trial { attempt }) = trial.candidate() else {
            return Err(invalid("Updater trial reservation is absent").into());
        };
        Ok(UnstartedUpdaterTrial {
            prepared: self,
            snapshot,
            attempt: attempt.clone(),
            closed: false,
        })
    }
}

impl UnstartedUpdaterTrial {
    /// Correlation for this exact attempted generation and journal revision, not health evidence.
    pub fn attempt(&self) -> &Trial {
        &self.attempt
    }

    /// Reauthenticate current metadata and publish this attempt before any native creation.
    /// `now` returns Unix UTC seconds. The caller retains this owner after success or error,
    /// because a failed storage completion may follow durable publication. Exact retries are safe.
    /// This method grants no native handles, executable path or permission to accept the engine.
    /// # Errors
    /// Rejects expired/revoked authority, clock rollback, changed journals, cancellation or closure.
    pub fn publish(
        &mut self,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        if self.closed {
            return Err(invalid("Updater creation admission is closed").into());
        }
        let source = &self.prepared.source;
        let bytes = source
            .payload
            .download_identity()
            .ok_or_else(|| invalid("Updater archive custody absent"))?;
        source.offer.authorization(&source.trust).publish_updater(
            bytes,
            now,
            context,
            |_, recheck| {
                recheck()?;
                source.trust.begin_updater_trial(&self.snapshot, context).map_err(io::Error::other)
            },
        )
    }

    /// Permanently close this never-started attempt and restore its verified prepared generation.
    /// A failed restore retains custody and can be retried; publication stays closed after any call.
    /// The original lock and exact journal identities, not elapsed time or process IDs, authorize it.
    /// # Errors
    /// Rejects unrelated journal changes, unsafe storage and cancellation without losing custody.
    pub fn restore(&mut self, context: &JobContext) -> Result<(), TrustFailure> {
        self.closed = true;
        self.prepared.source.trust.restore_unstarted_updater(&self.snapshot, context)
    }
}

#[cfg(test)]
mod tests;
