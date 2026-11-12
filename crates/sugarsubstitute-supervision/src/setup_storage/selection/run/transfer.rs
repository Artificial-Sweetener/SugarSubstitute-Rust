// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer an admitted trial's live ownership and protected snapshot across native process entry.
mod custody;
mod witness;
use super::*;
use crate::{
    durable_storage::files::Directory,
    setup_storage::{
        AdmittedCandidate, application::VerifiedApplication, check, disk_inheritance::capture_disk,
    },
};
use std::{
    io,
    os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle},
    path::{Path, PathBuf},
};
pub use witness::{TRIAL_WITNESS_RESERVATION_BYTES, TrialWitness, TrialWitnessTransfer};

/// Creator custody for one explicit native HANDLE_LIST, ordered as run then snapshot.
/// Consumes the original run and candidate so the creator cannot accept or reject the trial
/// after handing it off. Retain through process creation; no creation or health is inferred here.
/// The separate read-only snapshot cannot block publication of the actual selection record.
/// Live run custody prevents another trial; Windows removes the snapshot on final handle closure.
pub struct TrialTransfer {
    candidate: AdmittedCandidate,
    custody: custody::Transfer,
}
impl TrialTransfer {
    /// Exact pinned candidate directory used for process creation.
    pub fn directory(&self) -> &Path {
        self.candidate.directory()
    }

    /// Candidate executable; retain this owner through process creation to keep its bytes pinned.
    pub fn executable(&self) -> PathBuf {
        self.directory().join("sugarsubstitute.exe")
    }

    /// Borrow exactly these two inheritable handles in this order, without ambient inheritance.
    pub fn handles(&self) -> [BorrowedHandle<'_>; 2] {
        self.custody.handles()
    }

    /// Versioned trial arguments valid only with the two actual inherited handles.
    pub fn arguments(&self) -> [std::ffi::OsString; 3] {
        [
            sugarsubstitute_contracts::application_launch::TRIAL_LAUNCH_ARGUMENT.into(),
            (self.handles()[0].as_raw_handle() as usize).to_string().into(),
            (self.handles()[1].as_raw_handle() as usize).to_string().into(),
        ]
    }
}

/// Untrusted initial-entry handles, owned once and narrowed before any descendants are created.
/// They grant no trial authority until matched to protected native installation state.
pub struct InheritedTrial {
    run: File,
    snapshot: File,
}

/// Capture distinct run and snapshot handles from an explicit initial-entry transfer.
/// No disk paths or selection data are trusted here. Rejected handles are never used as paths.
/// # Safety
/// Call once at executable entry before local handle-owning resources or workers exist.
/// Both slots must be uniquely unowned by Rust/native owners and protected from concurrent
/// closure/reuse. Successful adoption consumes each slot even if a later slot is rejected.
/// # Errors
/// Rejects aliases, invalid, non-inheritable, standard or non-disk handles and native failures.
pub unsafe fn capture_trial_custody(handles: [usize; 2]) -> io::Result<InheritedTrial> {
    if handles[0] == handles[1] {
        return Err(crate::setup_storage::invalid("Trial transfer aliases its handle roles"));
    }
    // SAFETY: the initial-entry caller guarantees exclusive once-only ownership of both slots.
    let run = unsafe { capture_disk(handles[0]) }?;
    // SAFETY: the slots are distinct and the caller supplies the same ownership guarantee.
    let snapshot = unsafe { capture_disk(handles[1]) }?;
    Ok(InheritedTrial { run, snapshot })
}

/// Verified candidate bytes with the received run and original attempt retained through teardown.
/// This is ownership evidence, not startup health or evidence that its process family has exited.
pub struct TrialApplication {
    application: VerifiedApplication,
    run: InstallationRun,
    trial: OwnedTrial,
}
impl TrialApplication {
    /// Exact attempt for binding native startup observations.
    pub fn attempt(&self) -> &Trial {
        self.trial.attempt()
    }

    /// Verified generation directory; all payload files remain pinned by this owner.
    pub fn directory(&self) -> &Path {
        self.application.directory.path()
    }

    /// Accept after the caller observes authenticated native readiness for this exact attempt.
    /// Keep this owner through subsequent process-family teardown, including on publication error.
    /// # Errors
    /// Preserves changed state, cancellation and publication failures; duplicate acceptance fails.
    pub fn accept(&self, context: &JobContext) -> Result<InstallationRecord, SelectionFailure> {
        self.run.accept(&self.trial, context)
    }

    /// Publish or retry shell access after this exact trial has been health-accepted.
    /// Uses retained verified payloads and run custody; network access is unnecessary.
    /// Run on an admitted storage worker. Publication is independent of health acceptance:
    /// failure leaves the healthy selection intact and can be retried with this owner.
    /// # Errors
    /// Rejects unaccepted/stale trials, competing setup and edited or unknown shell files.
    pub fn repair_launch_entry(
        &self,
        context: &JobContext,
    ) -> Result<crate::setup_storage::InstalledApplication, SelectionFailure> {
        super::launch::entrypoint::repair(&self.run.custody, &self.application, context)
    }

    /// Reject only after actual native custody proves the candidate's family has settled.
    /// # Errors
    /// Preserves changed state, cancellation and publication failures; acceptance cannot be undone.
    pub fn reject(&self, context: &JobContext) -> Result<InstallationRecord, SelectionFailure> {
        self.run.reject(&self.trial, context)
    }

    /// Resolve this attempt after actual native process-family settlement, retaining acceptance
    /// that may have committed before a lost worker result. Repeated calls preserve preparation.
    /// # Errors
    /// Preserves changed activation history, cancellation and native publication failures.
    pub fn finish_settled(
        &self,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        self.run.finish_trial(&self.trial, context)
    }
}

#[cfg(test)]
mod shell_tests;
#[cfg(test)]
mod witness_tests;
#[cfg(test)]
mod witness_worker_tests;

impl InstallationRun {
    /// Consume the admitted candidate and its actual run into process-creation custody.
    /// Run custody excludes another trial; a separate snapshot binds the original record.
    /// No process starts, and creator custody cannot block the receiver's atomic acceptance.
    /// # Errors
    /// Rejects a foreign token, changed selection or native duplication failure without publication.
    pub fn transfer_candidate(
        self,
        candidate: AdmittedCandidate,
        context: &JobContext,
    ) -> Result<TrialTransfer, SelectionFailure> {
        check(context)?;
        self.check_trial(candidate.trial())?;
        candidate.require_trial_launch()?;
        let custody = custody::Transfer::new(candidate.trial(), context)?;
        Ok(TrialTransfer { candidate, custody })
    }

    /// Authenticate transferred custody and pin the exact candidate at the receiving coordinator.
    /// The caller supplies its already authenticated running generation. Disk state alone cannot
    /// enter this path: the live exclusive native object and protected snapshot are required.
    /// Reserve `LAUNCH_RESERVATION_BYTES` on the original admitted worker.
    /// # Errors
    /// Rejects foreign/nonexclusive run handles, changed selection, non-trial state, altered bytes,
    /// a different running generation, protection failures and cancellation. Never publishes state.
    pub fn receive_trial(
        store: Arc<SelectionStore>,
        inherited: InheritedTrial,
        running_generation: &Path,
        context: &JobContext,
    ) -> Result<TrialApplication, SelectionFailure> {
        let (run, trial) = custody::receive(store, inherited, context)?;
        let application = VerifiedApplication::open(
            &run.custody.store._installation,
            trial.attempt.generation.clone(),
            context,
        )?;
        application.payload.compatibility.require_trial_launch()?;
        let running = Directory::open(running_generation, false)?;
        if running.identity()? != application.directory.identity()? {
            return Err(SelectionFailure::RunOwnership);
        }
        check(context)?;
        Ok(TrialApplication { application, run, trial })
    }
}
