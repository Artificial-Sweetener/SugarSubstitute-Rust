// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind activation to live OS custody and a non-serializable current-attempt token.
use super::{SelectionFailure, SelectionStore};
mod launch;
mod recovery;
mod settlement;
mod unstarted;
pub(crate) use unstarted::UnstartedTrial;
#[allow(unsafe_code)]
mod transfer;
pub use launch::{AcceptedApplication, InstallationLaunch};
use std::{fs::File, sync::Arc};
use sugarsubstitute_contracts::{
    execution::JobContext,
    installation::{Candidate, InstallationRecord, Trial},
};
pub use transfer::{
    InheritedTrial, TRIAL_WITNESS_RESERVATION_BYTES, TrialApplication, TrialTransfer, TrialWitness,
    TrialWitnessTransfer, capture_trial_custody,
};

const LOCK: &str = ".installation-run-lock";

/// Hold through the owning application's entire process-family teardown.
/// This installation guard is separate from application/Comfy resource leases and brief
/// selection writes. It never steals a lock using elapsed time, PID guesses or file age.
/// Native file-sharing exclusion remains attached to every duplicate until final closure.
/// Acquiring it does not prove that descendants of an earlier owner have settled.
pub struct InstallationRun {
    custody: Arc<Custody>,
    issued: bool,
}
struct Custody {
    _lock: File,
    store: Arc<SelectionStore>,
}

/// Live authority for one trial admitted by this run, retained until its caller retires it.
/// Cannot be reconstructed from disk or a claimed health receipt. It retains OS exclusion
/// even if the run handle is dropped. It proves ownership, not health or process settlement.
pub struct OwnedTrial {
    custody: Arc<Custody>,
    snapshot: InstallationRecord,
    attempt: Trial,
}
impl OwnedTrial {
    /// Exact correlation data for authenticated native launch/readiness messages.
    pub fn attempt(&self) -> &Trial {
        &self.attempt
    }
}
impl InstallationRun {
    /// Recheck retained launcher compatibility while run custody excludes its replacement.
    pub(in crate::setup_storage) fn require_launcher(
        &self,
        compatibility: &crate::setup_storage::compatibility::Compatibility,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        self.custody.store.require_launcher(compatibility, context)
    }

    /// Acquire live installation exclusion on an admitted storage job without waiting.
    /// Retain the returned owner until all application-owned processes are settled.
    /// # Errors
    /// Rejects a competing run, unknown/nonempty lock contents, invalid protection or selection.
    pub fn acquire(
        store: Arc<SelectionStore>,
        context: &JobContext,
    ) -> Result<Self, SelectionFailure> {
        let lock = store.protected(context, |protection| {
            let lock = protection
                .transferable_rendezvous(&store.root.path().join(LOCK), context)
                .map_err(classify_run_open)?;
            let _reader = super::reader(&store.root, protection, context)?;
            store.read(protection)?.ok_or(SelectionFailure::Missing)?;
            Ok(lock)
        })?;
        Ok(Self { custody: Arc::new(Custody { _lock: lock, store }), issued: false })
    }

    /// Persist the prepared candidate's exact attempt before starting its process family.
    /// The caller must revalidate package authority and compatibility before this call.
    /// Only one trial may be admitted during this run's ownership.
    /// # Errors
    /// Rejects stale selection, unfinished preparation, an earlier trial or repeated admission.
    pub(in crate::setup_storage) fn begin_trial(
        &mut self,
        expected: &InstallationRecord,
        context: &JobContext,
    ) -> Result<OwnedTrial, SelectionFailure> {
        if self.issued {
            return Err(SelectionFailure::RunOwnership);
        }
        let snapshot = expected.begin_trial()?;
        let Some(Candidate::Trial { attempt }) = snapshot.candidate() else {
            return Err(SelectionFailure::RunOwnership);
        };
        let attempt = attempt.clone();
        self.custody.store.replace(expected, &snapshot, context)?;
        self.issued = true;
        Ok(OwnedTrial { custody: Arc::clone(&self.custody), snapshot, attempt })
    }

    /// Bind package authorization to the exact native installation held by this run.
    pub(in crate::setup_storage) fn validate_installation(
        &self,
        installation: &super::Directory,
    ) -> Result<(), SelectionFailure> {
        if installation.identity()? != self.custody.store._installation.identity()? {
            return Err(SelectionFailure::RunOwnership);
        }
        Ok(())
    }

    /// Commit the owned attempt after authenticated native readiness confirms application health.
    /// The caller supplies that observation; a token alone is not health evidence.
    /// # Errors
    /// Rejects another run's token, stale or duplicate completion and native publication failure.
    pub fn accept(
        &self,
        trial: &OwnedTrial,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        self.check_trial(trial)?;
        let next = trial.snapshot.accept(&trial.attempt)?;
        self.custody.store.replace(&trial.snapshot, &next, context)?;
        Ok(next)
    }

    /// Reject the owned attempt after actual native custody confirms its family has settled.
    /// The accepted generation remains selected and this package is quarantined from auto retry.
    /// # Errors
    /// Rejects another run's token, stale or duplicate completion and native publication failure.
    pub fn reject(
        &self,
        trial: &OwnedTrial,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        self.check_trial(trial)?;
        let next = trial.snapshot.reject(&trial.attempt)?;
        self.custody.store.replace(&trial.snapshot, &next, context)?;
        Ok(next)
    }

    fn check_trial(&self, trial: &OwnedTrial) -> Result<(), SelectionFailure> {
        if !Arc::ptr_eq(&self.custody, &trial.custody) {
            return Err(SelectionFailure::RunOwnership);
        }
        Ok(())
    }
}

/// Classify sharing contention only at this exact run-rendezvous open. Protection retirement,
/// malformed files, access denial and other storage failures preserve their original meaning.
fn classify_run_open(cause: std::io::Error) -> SelectionFailure {
    if cause.get_ref().and_then(|cause| cause.downcast_ref::<windows::core::Error>()).is_some_and(
        |cause| {
            cause.code()
                == windows::core::HRESULT::from_win32(
                    windows::Win32::Foundation::ERROR_SHARING_VIOLATION.0,
                )
        },
    ) {
        SelectionFailure::RunBusy(cause)
    } else {
        SelectionFailure::Storage(cause)
    }
}

#[cfg(test)]
mod tests;
