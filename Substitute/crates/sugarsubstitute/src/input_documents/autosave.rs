// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coalesce bursts of input-document changes into one save, and save only changed content.
//!
//! The host reports each change as a new content generation with the time it happened, polls
//! at [`Autosave::deadline`], and observes the save job after draining execution completions.
//! A save starts once changes have been quiet for [`AutosavePolicy::quiet`], or after
//! [`AutosavePolicy::longest`] while changes keep arriving. One save runs at a time; changes
//! during a save wait for the next one. Content whose fingerprint matches the last saved or
//! restored content is not written again. A failed or stopped save leaves the content unsaved
//! and retries after the quiet period. At close, [`Autosave::flush`] saves whatever is unsaved at
//! once within the shutdown bound.
use super::{InputDocumentStore, SaveReceipt, StoreError};
use crate::job_sequence::JobSequence;
use std::{error::Error, fmt, time::Duration};
use sugarsubstitute_canvas::{ArchiveError, InputDocumentArchive};
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionPort, IdentityError, JobClass, JobOutcome, JobSpec, Moment, Operation,
    OwnedJob, Scope, TerminalKind, TimeError,
};

/// Debounce timing for automatic saves. A save has no cutoff of its own: it ends with its result,
/// an observed failure or cancellation, and only [`Autosave::flush`] is bounded, by the shutdown
/// cutoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutosavePolicy {
    /// Quiet period after the latest change before a save starts.
    pub quiet: Duration,
    /// Longest a change waits while further changes keep arriving.
    pub longest: Duration,
}

/// Deterministic debounce over host-assigned, increasing content generations.
#[derive(Debug)]
pub struct AutosaveSchedule {
    policy: AutosavePolicy,
    saved: u64,
    latest: u64,
    first: Option<Moment>,
    due: Option<Moment>,
    saving: Option<u64>,
}
impl AutosaveSchedule {
    /// Begin with `saved` as the generation already on disk, such as the restored content.
    pub const fn new(policy: AutosavePolicy, saved: u64) -> Self {
        Self { policy, saved, latest: saved, first: None, due: None, saving: None }
    }

    /// Record that content reached `generation` at `now`; older or repeated generations are
    /// ignored.
    /// # Errors
    /// Rejects deadlines beyond the monotonic clock's range.
    pub fn changed(&mut self, generation: u64, now: Moment) -> Result<(), TimeError> {
        if generation <= self.latest {
            return Ok(());
        }
        self.latest = generation;
        let first = *self.first.get_or_insert(now);
        self.due =
            Some(now.checked_add(self.policy.quiet)?.min(first.checked_add(self.policy.longest)?));
        Ok(())
    }

    /// When the host should next poll; `None` while nothing waits or a save is running.
    pub fn deadline(&self) -> Option<Moment> {
        self.saving.is_none().then_some(self.due).flatten()
    }

    /// Whether content newer than the saved generation exists.
    pub const fn dirty(&self) -> bool {
        self.latest > self.saved
    }

    /// Start the due save, returning the generation to capture.
    pub fn begin(&mut self, now: Moment) -> Option<u64> {
        if self.saving.is_some() || !self.dirty() || self.due.is_none_or(|due| due > now) {
            return None;
        }
        self.saving = Some(self.latest);
        self.first = None;
        self.due = None;
        Some(self.latest)
    }

    /// Start a save of the latest generation now, whatever its debounce, such as at close.
    /// Returns `None` while nothing is unsaved or a save is running.
    pub fn flush(&mut self) -> Option<u64> {
        if self.saving.is_some() || !self.dirty() {
            return None;
        }
        self.saving = Some(self.latest);
        self.first = None;
        self.due = None;
        Some(self.latest)
    }

    /// Finish the running save; an unsaved generation is retried after the quiet period.
    /// # Errors
    /// Rejects deadlines beyond the monotonic clock's range.
    pub fn finished(&mut self, saved: bool, now: Moment) -> Result<(), TimeError> {
        let Some(generation) = self.saving.take() else { return Ok(()) };
        if saved {
            self.saved = self.saved.max(generation);
        } else if self.due.is_none() && self.dirty() {
            self.first = Some(now);
            self.due = Some(now.checked_add(self.policy.quiet)?);
        }
        Ok(())
    }
}

/// Why autosave could not proceed; unsaved content stays dirty.
#[derive(Debug)]
pub enum AutosaveError {
    /// Monotonic deadline arithmetic overflowed.
    Time(TimeError),
    /// Job identities are exhausted.
    Identity(IdentityError),
    /// The host could not capture its documents.
    Capture(ArchiveError),
    /// Execution refused the save job.
    Admission(AdmissionError),
}
impl fmt::Display for AutosaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Time(cause) => write!(formatter, "Autosave deadline failed: {cause:?}"),
            Self::Identity(cause) => write!(formatter, "Autosave job identity failed: {cause:?}"),
            Self::Capture(cause) => write!(formatter, "Autosave capture failed: {cause}"),
            Self::Admission(cause) => write!(formatter, "Autosave admission failed: {cause}"),
        }
    }
}
impl Error for AutosaveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Capture(cause) => Some(cause),
            Self::Admission(cause) => Some(cause),
            Self::Time(_) | Self::Identity(_) => None,
        }
    }
}
impl From<TimeError> for AutosaveError {
    fn from(cause: TimeError) -> Self {
        Self::Time(cause)
    }
}

/// Terminal result of one automatic save.
#[derive(Debug)]
pub enum SaveOutcome {
    /// The content was written.
    Saved(SaveReceipt),
    /// The content matched what is already on disk; nothing was written.
    Unchanged,
    /// The save failed; the content stays unsaved.
    Failed(StoreError),
    /// The save was cancelled, stopped at the shutdown cutoff or panicked; the content stays
    /// unsaved.
    Stopped(TerminalKind),
}

struct InFlight {
    fingerprint: [u8; 32],
    job: OwnedJob<SaveReceipt, StoreError>,
}

/// Debounced automatic saving through one store and the execution owner.
pub struct Autosave {
    store: InputDocumentStore,
    schedule: AutosaveSchedule,
    saved: Option<[u8; 32]>,
    running: Option<InFlight>,
}
impl Autosave {
    /// Start clean at generation `saved`, whose content is `restored` when it came from disk.
    pub fn new(
        store: InputDocumentStore,
        policy: AutosavePolicy,
        saved: u64,
        restored: Option<&InputDocumentArchive>,
    ) -> Self {
        Self {
            store,
            schedule: AutosaveSchedule::new(policy, saved),
            saved: restored.map(InputDocumentArchive::fingerprint),
            running: None,
        }
    }

    /// Record a document change; see [`AutosaveSchedule::changed`].
    /// # Errors
    /// Rejects deadlines beyond the monotonic clock's range.
    pub fn changed(&mut self, generation: u64, now: Moment) -> Result<(), AutosaveError> {
        Ok(self.schedule.changed(generation, now)?)
    }

    /// When the host should next call [`Self::poll`].
    pub fn deadline(&self) -> Option<Moment> {
        self.schedule.deadline()
    }

    /// Whether a save job awaits its observed terminal outcome.
    pub const fn saving(&self) -> bool {
        self.running.is_some()
    }

    /// Whether content newer than the last saved generation exists.
    pub const fn dirty(&self) -> bool {
        self.schedule.dirty()
    }

    /// When a save is due, capture the documents and submit their save. Returns
    /// [`SaveOutcome::Unchanged`] when the captured content is already on disk.
    /// # Errors
    /// Preserves capture, identity and admission failures; the content stays dirty and is
    /// retried after the quiet period.
    pub fn poll<P: ExecutionPort>(
        &mut self,
        now: Moment,
        execution: &P,
        scope: &Scope,
        jobs: &JobSequence,
        capture: impl FnOnce() -> Result<InputDocumentArchive, ArchiveError>,
    ) -> Result<Option<SaveOutcome>, AutosaveError> {
        if self.schedule.begin(now).is_none() {
            return Ok(None);
        }
        self.start(now, None, execution, scope, jobs, capture)
    }

    /// Save every unsaved change now, regardless of the debounce, with a job deadline at
    /// `cutoff`, such as the bounded shutdown's. A save that cannot finish by then is stopped
    /// before it replaces the current archive, which stays intact.
    /// # Errors
    /// Preserves capture, identity and admission failures; the content stays dirty.
    pub fn flush<P: ExecutionPort>(
        &mut self,
        now: Moment,
        cutoff: Moment,
        execution: &P,
        scope: &Scope,
        jobs: &JobSequence,
        capture: impl FnOnce() -> Result<InputDocumentArchive, ArchiveError>,
    ) -> Result<Option<SaveOutcome>, AutosaveError> {
        if self.schedule.flush().is_none() {
            return Ok(None);
        }
        self.start(now, Some(cutoff), execution, scope, jobs, capture)
    }

    /// Submit the save the schedule just began, recording a failure to start as unsaved.
    fn start<P: ExecutionPort>(
        &mut self,
        now: Moment,
        deadline: Option<Moment>,
        execution: &P,
        scope: &Scope,
        jobs: &JobSequence,
        capture: impl FnOnce() -> Result<InputDocumentArchive, ArchiveError>,
    ) -> Result<Option<SaveOutcome>, AutosaveError> {
        let submitted = self.submit(deadline, execution, scope, jobs, capture);
        match submitted {
            Ok(Some(running)) => {
                self.running = Some(running);
                Ok(None)
            }
            Ok(None) => {
                self.schedule.finished(true, now)?;
                Ok(Some(SaveOutcome::Unchanged))
            }
            Err(cause) => {
                self.schedule.finished(false, now)?;
                Err(cause)
            }
        }
    }

    /// Observe the running save after the host drained execution completions.
    /// # Errors
    /// Rejects deadlines beyond the monotonic clock's range.
    pub fn observe(&mut self, now: Moment) -> Result<Option<SaveOutcome>, AutosaveError> {
        let Some(running) = &mut self.running else { return Ok(None) };
        let Some(completion) = running.job.try_take_completion() else { return Ok(None) };
        let fingerprint = running.fingerprint;
        self.running = None;
        let outcome = match completion.outcome {
            JobOutcome::Success(receipt) => {
                self.saved = Some(fingerprint);
                SaveOutcome::Saved(receipt)
            }
            JobOutcome::Failure(cause) => SaveOutcome::Failed(cause),
            other => SaveOutcome::Stopped(other.kind()),
        };
        self.schedule.finished(matches!(outcome, SaveOutcome::Saved(_)), now)?;
        if let SaveOutcome::Failed(cause) = &outcome {
            tracing::warn!(operation = "input_documents_autosave", cause_chain = %crate::diagnostics::CauseChain(cause),
                "Input documents remain unsaved");
        }
        Ok(Some(outcome))
    }

    /// Request cancellation of the running save; observe its terminal outcome as usual.
    pub fn cancel(&self) {
        if let Some(running) = &self.running {
            running.job.cancel();
        }
    }

    fn submit<P: ExecutionPort>(
        &self,
        deadline: Option<Moment>,
        execution: &P,
        scope: &Scope,
        jobs: &JobSequence,
        capture: impl FnOnce() -> Result<InputDocumentArchive, ArchiveError>,
    ) -> Result<Option<InFlight>, AutosaveError> {
        let archive = capture().map_err(AutosaveError::Capture)?;
        let fingerprint = archive.fingerprint();
        if self.saved == Some(fingerprint) {
            return Ok(None);
        }
        let spec = JobSpec {
            id: jobs.allocate().map_err(AutosaveError::Identity)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline,
            reservation_bytes: InputDocumentStore::save_reservation(&archive),
            replacement: None,
        };
        let job = self
            .store
            .submit_save(execution, scope, spec, archive)
            .map_err(AutosaveError::Admission)?;
        Ok(Some(InFlight { fingerprint, job }))
    }
}
