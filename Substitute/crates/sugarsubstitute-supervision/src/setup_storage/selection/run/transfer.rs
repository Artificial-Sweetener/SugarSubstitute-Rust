// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer an admitted trial's live ownership and protected snapshot across native process entry.
mod custody;
mod witness;
use super::*;
use crate::{
    durable_storage::files::Directory,
    setup_storage::{
        AdmittedCandidate,
        application::VerifiedApplication,
        check,
        disk_inheritance::{capture_disk, capture_event},
    },
};
use std::{
    fs::File,
    io,
    os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    time::Duration,
};
use sugarsubstitute_contracts::installation::{TrialVerdict, Undecided};
use windows::Win32::{
    Foundation::HANDLE,
    Security::SECURITY_ATTRIBUTES,
    System::Threading::{CreateEventW, SetEvent},
};
pub use witness::{TRIAL_WITNESS_RESERVATION_BYTES, TrialWitness, TrialWitnessTransfer};

/// Longest time a trial creator gives its child to hold its own run slot after creation.
/// The child adopts once its coordinator is elected, before verifying payloads. The first start
/// of a freshly updated executable on a slow disk, with antivirus scanning it, can take many
/// seconds, and a child ended too early would retry the update at every launch without ever
/// applying it; a longer silence means it is stuck, and the creator gives up its handoff.
pub const TRIAL_ADOPTION: Duration = Duration::from_secs(30);
/// After giving up the handoff, how long the creator still waits for a child that may have
/// adopted at the last moment to say so, or to exit, before ending it.
pub const TRIAL_ADOPTION_GRACE: Duration = Duration::from_secs(2);
/// Longest time a sender waits for an earlier sender in its chain to finish its handoff: a trial
/// creator waits at most its adoption bound, its grace and one more bound for termination.
const HANDOFF_WAIT: Duration =
    Duration::from_secs(TRIAL_ADOPTION.as_secs() * 2 + TRIAL_ADOPTION_GRACE.as_secs());

/// Creator custody for one explicit native HANDLE_LIST, ordered as run witness, snapshot, then
/// the slot-held event. Consumes the original run and candidate so the creator cannot accept or
/// reject the trial after handing it off. The creator holds its run slot and the handoff byte
/// until the child signals that it holds its own slot or is known to be gone; dropping this
/// owner releases both. The separate read-only snapshot cannot block publication of the actual
/// selection record. Windows removes the snapshot on final handle closure.
pub struct TrialTransfer {
    candidate: AdmittedCandidate,
    custody: custody::Transfer,
    adopted: OwnedHandle,
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

    /// Borrow exactly these three inheritable handles in this order, without ambient inheritance.
    pub fn handles(&self) -> [BorrowedHandle<'_>; 3] {
        let [run, snapshot] = self.custody.handles();
        [run, snapshot, self.adopted.as_handle()]
    }

    /// Versioned trial arguments valid only with the three actual inherited handles.
    pub fn arguments(&self) -> [std::ffi::OsString; 4] {
        let handles = self.handles().map(|handle| handle.as_raw_handle() as usize);
        [
            sugarsubstitute_contracts::application_launch::TRIAL_LAUNCH_ARGUMENT.into(),
            handles[0].to_string().into(),
            handles[1].to_string().into(),
            handles[2].to_string().into(),
        ]
    }

    /// Manual-reset event the child signals once it holds its own run slot. Wait on it together
    /// with the child's process; it stays signalled once set.
    pub fn adoption(&self) -> BorrowedHandle<'_> {
        self.adopted.as_handle()
    }

    /// Give up the handoff: a child that has not yet confirmed its slot now refuses the trial,
    /// while this creator keeps run exclusion until it is dropped.
    pub fn release_handoff(&self) {
        self.custody.release_handoff();
    }

    /// Resolve a trial whose child was created but never held its run slot and has now
    /// exited, under this creator's continuously held run. A child that stayed silent until its
    /// creator ended it, as when antivirus holds the new files, returns the candidate to prepared
    /// with no verdict and nothing against the release. A child that exited on its own before
    /// taking over is an ending nobody saw the cause of: the candidate returns to prepared, but
    /// it counts toward the same streak as an abandoned trial, so a release that cannot start
    /// at all is not retried at every launch forever. Repeated calls keep the first result.
    /// Call only after observing the child's exit, on an admitted storage worker reserving
    /// `SELECTION_PUBLICATION_RESERVATION_BYTES`.
    /// # Errors
    /// Preserves changed activation history, cancellation and native publication failures.
    pub fn release_unadopted(
        &self,
        ending: UnadoptedEnding,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        let trial = self.candidate.trial();
        match ending {
            UnadoptedEnding::Ended => {
                let run = InstallationRun { custody: Arc::clone(&trial.custody), issued: true };
                run.finish_trial(trial, TrialVerdict::NoVerdict(Undecided::Unadopted), context)
            }
            UnadoptedEnding::Exited => {
                let run = InstallationRun { custody: Arc::clone(&trial.custody), issued: false };
                settlement::through_contention(context, || run.recover_abandoned_trial(context))
            }
        }
    }
}

/// How a created trial child that never held its run slot ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnadoptedEnding {
    /// It exited on its own before taking over.
    Exited,
    /// It stayed silent past its bound and grace and its creator ended it.
    Ended,
}

/// Create the inheritable, unsignalled manual-reset event a trial child sets once it holds its
/// own run slot.
fn adoption_event() -> io::Result<OwnedHandle> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    // SAFETY: a private unnamed manual-reset event; the attributes outlive the call and
    // OwnedHandle adopts the returned handle immediately.
    let event =
        unsafe { CreateEventW(Some(&attributes), true, false, None) }.map_err(io::Error::other)?;
    // SAFETY: the event was just created and has no other owner.
    Ok(unsafe { OwnedHandle::from_raw_handle(event.0) })
}

/// Tell the creator that this process holds its own run slot.
fn signal_adopted(event: &OwnedHandle) -> io::Result<()> {
    // SAFETY: the adopted event handle is live for this call.
    unsafe { SetEvent(HANDLE(event.as_raw_handle())) }.map_err(io::Error::other)
}

/// Untrusted initial-entry handles, owned once and narrowed before any descendants are created.
/// They grant no trial authority until matched to protected native installation state.
pub struct InheritedTrial {
    run: File,
    snapshot: File,
    /// Signalled once this process holds its own run slot; absent for the custody witness,
    /// whose sender learns of adoption through its authenticated channel instead.
    adopted: Option<OwnedHandle>,
}

/// Capture distinct run witness, snapshot and slot-held event handles from an explicit
/// initial-entry transfer. No disk paths or selection data are trusted here. Rejected handles
/// are never used as paths.
/// # Safety
/// Call once at executable entry before local handle-owning resources or workers exist.
/// All three slots must be uniquely unowned by Rust/native owners and protected from concurrent
/// closure/reuse. Successful adoption consumes each slot even if a later slot is rejected.
/// # Errors
/// Rejects aliases, invalid, non-inheritable, standard or wrongly typed handles and native
/// failures.
pub unsafe fn capture_trial_custody(handles: [usize; 3]) -> io::Result<InheritedTrial> {
    if handles[0] == handles[1] || handles[0] == handles[2] || handles[1] == handles[2] {
        return Err(crate::setup_storage::invalid("Trial transfer aliases its handle roles"));
    }
    // SAFETY: the initial-entry caller guarantees exclusive once-only ownership of every slot.
    let run = unsafe { capture_disk(handles[0]) }?;
    // SAFETY: the slots are distinct and the caller supplies the same ownership guarantee.
    let snapshot = unsafe { capture_disk(handles[1]) }?;
    // SAFETY: as above, for the third distinct slot.
    let adopted = unsafe { capture_event(handles[2]) }?;
    Ok(InheritedTrial { run, snapshot, adopted: Some(adopted) })
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

    /// Resolve this attempt with the coordinator's classified `verdict`, retaining acceptance
    /// that may have committed before a lost worker result and any earlier settlement of this
    /// attempt. Repeated calls preserve preparation.
    /// # Errors
    /// Preserves changed activation history, cancellation and native publication failures.
    pub fn finish_settled(
        &self,
        verdict: sugarsubstitute_contracts::installation::TrialVerdict,
        context: &JobContext,
    ) -> Result<InstallationRecord, SelectionFailure> {
        self.run.finish_trial(&self.trial, verdict, context)
    }
}

#[cfg(test)]
mod shell_tests;
#[cfg(test)]
mod witness_tests;
#[cfg(test)]
mod witness_worker_tests;

impl InstallationRun {
    /// Consume the admitted candidate and its actual run into process-creation custody, holding
    /// the run's handoff byte so the child can adopt it. Run custody excludes another trial; a
    /// separate snapshot binds the original record. No process starts, and creator custody
    /// cannot block the receiver's atomic acceptance. Run on a blocking infrastructure job.
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
        let adopted = adoption_event()?;
        Ok(TrialTransfer { candidate, custody, adopted })
    }

    /// Adopt transferred custody and pin the exact candidate at the receiving coordinator.
    /// The caller supplies its already authenticated running generation. Disk state alone cannot
    /// enter this path: a live sender's handoff and the protected snapshot are required. Once
    /// this process holds its own run slot and the snapshot matches the record, it signals the
    /// creator, before payload verification, so the creator may exit.
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
