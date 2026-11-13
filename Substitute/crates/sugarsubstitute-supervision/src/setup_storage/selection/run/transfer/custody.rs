// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One native transfer boundary binds live run exclusion to the protected original trial.
//!
//! The sender holds its run slot and the handoff byte and passes a witness handle; the receiver
//! takes its own slot while the handoff byte is still held, so exclusion never lapses between
//! the two processes and outlives the sender's exit.
use super::*;
use crate::setup_storage::disk_inheritance::duplicate_inheritable;
use crate::setup_storage::selection::snapshot;

/// Keep original custody alongside the exact two independently owned transfer handles. The
/// sender's handoff stays held while original custody lives, until `release_handoff`.
pub(super) struct Transfer {
    _custody: Arc<Custody>,
    run: File,
    snapshot: File,
}
impl Transfer {
    /// Copy only an unresolved, unchanged trial while its original run still owns exclusion.
    pub(super) fn new(trial: &OwnedTrial, context: &JobContext) -> Result<Self, SelectionFailure> {
        check(context)?;
        let store = &trial.custody.store;
        let snapshot = store.protected(context, |protection| {
            let _reader = super::super::super::reader(&store.root, protection, context)?;
            let record = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if record != trial.snapshot {
                return Err(SelectionFailure::Changed);
            }
            Ok(snapshot::create(store, protection, &record, snapshot::Purpose::Trial, context)?)
        })?;
        let until = context.now().checked_add(super::HANDOFF_WAIT).map_err(io::Error::other)?;
        trial.custody.lock.hold_handoff(context, until).map_err(|cause| {
            if crate::durable_storage::protection::is_busy(&cause) {
                SelectionFailure::RunBusy(cause)
            } else {
                SelectionFailure::Storage(cause)
            }
        })?;
        let run = duplicate_inheritable(trial.custody.lock.witness())?;
        check(context)?;
        Ok(Self { _custody: Arc::clone(&trial.custody), run, snapshot })
    }

    /// End the sender's handoff while keeping its own run slot.
    pub(super) fn release_handoff(&self) {
        self._custody.lock.release_handoff();
    }

    /// Borrow the run then immutable snapshot for explicit native transfer only.
    pub(super) fn handles(&self) -> [BorrowedHandle<'_>; 2] {
        [self.run.as_handle(), self.snapshot.as_handle()]
    }
}

/// Adopt the sender's exclusion into this process's own slot and authenticate the exact current
/// snapshot before creating local authority, then tell a waiting creator that the slot is held.
/// Neither a fresh acquisition nor a reconstructed record can enter through this boundary.
pub(super) fn receive(
    store: Arc<SelectionStore>,
    inherited: InheritedTrial,
    context: &JobContext,
) -> Result<(InstallationRun, OwnedTrial), SelectionFailure> {
    check(context)?;
    let InheritedTrial { run, snapshot: transferred, adopted } = inherited;
    let (snapshot, lock) = store.protected(context, |protection| {
        let lock = protection.adopt_exclusion(run, &store.root.path().join(LOCK), context)?;
        let transferred =
            snapshot::read(&store, &transferred, snapshot::Purpose::Trial, protection, context)?;
        let _reader = super::super::super::reader(&store.root, protection, context)?;
        let record = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
        if transferred != record {
            return Err(SelectionFailure::Changed);
        }
        Ok((record, lock))
    })?;
    let Some(Candidate::Trial { attempt }) = snapshot.candidate() else {
        return Err(SelectionFailure::RunOwnership);
    };
    let attempt = attempt.clone();
    check(context)?;
    if let Some(adopted) = adopted {
        super::signal_adopted(&adopted)?;
    }
    let custody = Arc::new(Custody { lock, store });
    let run = InstallationRun { custody: Arc::clone(&custody), issued: true };
    let trial = OwnedTrial { custody, snapshot, attempt };
    Ok((run, trial))
}
