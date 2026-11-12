// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Repair shell access using the same verified application authority used for offline startup.
use super::*;
use crate::setup_storage::{InstalledApplication, entrypoint, transaction::admission::Admission};

impl AcceptedApplication {
    /// Restore shell access to this accepted application and return the published launch location.
    /// Run on an admitted storage worker, retaining this owner through the operation.
    /// Network access and cached downloads are unnecessary. This does not accept an update,
    /// repair application files, or establish that any earlier process family has settled.
    /// # Errors
    /// Preserves edited/unknown links, conflicting maintenance, interrupted trials and stale
    /// selection. An interrupted publication leaves a complete old or new link for retry.
    pub fn repair_launch_entry(
        &self,
        context: &JobContext,
    ) -> Result<InstalledApplication, SelectionFailure> {
        repair(&self.custody, &self.application, context)
    }

    /// Reuse the installer's original mutation owner rather than recursively locking setup.
    pub(in crate::setup_storage) fn repair_under_admission(
        &self,
        admission: &Admission,
        context: &JobContext,
    ) -> Result<InstalledApplication, SelectionFailure> {
        repair_under_admission(&self.custody, &self.application, admission, context)
    }
}

/// Share shell publication between an accepted launch and a newly accepted live trial.
pub(in crate::setup_storage::selection::run) fn repair(
    custody: &Custody,
    application: &VerifiedApplication,
    context: &JobContext,
) -> Result<InstalledApplication, SelectionFailure> {
    check(context)?;
    let admission = Admission::open(custody.store._installation.path(), false, context)?;
    repair_under_admission(custody, application, &admission, context)
}

/// Recheck accepted authority under the same writer used by selection publication.
fn repair_under_admission(
    custody: &Custody,
    application: &VerifiedApplication,
    admission: &Admission,
    context: &JobContext,
) -> Result<InstalledApplication, SelectionFailure> {
    check(context)?;
    let store = &custody.store;
    if admission.root.identity()? != store._installation.identity()? {
        return Err(SelectionFailure::RunOwnership);
    }
    store.protected(context, |protection| {
            let _writer = super::super::super::writer(&store.root, protection, context)?;
            let current = store.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if !matches!(current.startup_selection(), StartupSelection::Accepted(generation) if generation == &application.generation) {
                return Err(SelectionFailure::Changed);
            }
            if entrypoint::stable::repair(&admission.root, &store.root, application, protection, context)? {
                Ok(InstalledApplication {
                    executable: admission.root.path().join(entrypoint::stable::ENTRY),
                    directory: admission.root.path().to_path_buf(),
                })
            } else {
                entrypoint::repair(&admission.root, application.directory.path(), context)?;
                Ok(InstalledApplication {
                    executable: application.directory.path().join("sugarsubstitute.exe"),
                    directory: application.directory.path().to_path_buf(),
                })
            }
        })
}
