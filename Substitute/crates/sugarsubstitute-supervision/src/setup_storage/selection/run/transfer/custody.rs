// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! One native transfer boundary binds live run exclusion to the protected original trial.
use super::*;
use crate::setup_storage::disk_inheritance::duplicate_inheritable;
use crate::setup_storage::selection::snapshot;

/// Keep original custody alongside the exact two independently owned transfer handles.
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
        let run = duplicate_inheritable(&trial.custody._lock)?;
        check(context)?;
        Ok(Self { _custody: Arc::clone(&trial.custody), run, snapshot })
    }

    /// Borrow the run then immutable snapshot for explicit native transfer only.
    pub(super) fn handles(&self) -> [BorrowedHandle<'_>; 2] {
        [self.run.as_handle(), self.snapshot.as_handle()]
    }
}

/// Authenticate original exclusion and exact current snapshot before creating local authority.
/// Neither a reopened lock nor a reconstructed record can enter through this boundary.
pub(super) fn receive(
    store: Arc<SelectionStore>,
    inherited: InheritedTrial,
    context: &JobContext,
) -> Result<(InstallationRun, OwnedTrial), SelectionFailure> {
    check(context)?;
    let snapshot = store.protected(context, |protection| {
        protection.verify_transferable_rendezvous(
            &inherited.run,
            &store.root.path().join(LOCK),
            context,
        )?;
        let transferred = snapshot::read(
            &store,
            &inherited.snapshot,
            snapshot::Purpose::Trial,
            protection,
            context,
        )?;
        let _reader = super::super::super::reader(&store.root, protection, context)?;
        let record = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
        if transferred != record {
            return Err(SelectionFailure::Changed);
        }
        Ok(record)
    })?;
    let Some(Candidate::Trial { attempt }) = snapshot.candidate() else {
        return Err(SelectionFailure::RunOwnership);
    };
    let attempt = attempt.clone();
    check(context)?;
    let custody = Arc::new(Custody { _lock: inherited.run, store });
    let run = InstallationRun { custody: Arc::clone(&custody), issued: true };
    let trial = OwnedTrial { custody, snapshot, attempt };
    Ok((run, trial))
}
