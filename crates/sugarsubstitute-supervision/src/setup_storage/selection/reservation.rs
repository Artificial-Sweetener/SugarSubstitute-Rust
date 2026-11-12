// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin launch selection without taking the elected application's run exclusion.
use super::*;
use crate::setup_storage::application::VerifiedApplication;
use std::{path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::installation::{Generation, StartupSelection, Trial};
mod forwarding;

/// A launcher's verified next action without any claim to an application process lifetime.
pub enum ReservedLaunch {
    /// Verified selection and application bytes for accepted startup or an active trial's
    /// independent invocation. Only the original transferred run can admit that trial.
    Application(ReservedApplication),
    /// Installation has not yet accepted an application.
    NeedsInstallation,
    /// Native custody must reconcile this attempt before another launch can be reserved.
    ReconcileTrial(Trial),
}

/// Short-lived selection and executable custody for the stable launcher's startup transfer.
/// This is not application-run ownership, health evidence or process-settlement authority.
/// The elected coordinator can acquire its run lock while this reservation exists.
/// The selection travels as an immutable snapshot of the record value, so it never blocks a
/// concurrent replacement; the receiver compares it with the current record at its point of
/// use. A forwarding snapshot never grants primary admission. Retain custody through native
/// transfer; do not retain it for the running application's lifetime.
pub struct ReservedApplication {
    pub(super) application: VerifiedApplication,
    pub(super) _selection: File,
    pub(super) _store: Arc<SelectionStore>,
}
impl ReservedApplication {
    /// Exact selected identity covered by the retained snapshot and verified manifest.
    pub fn generation(&self) -> &Generation {
        &self.application.generation
    }
    /// Pinned generation directory for native launch composition.
    pub fn directory(&self) -> &Path {
        self.application.directory.path()
    }
    /// Absolute executable; this path does not substitute for retaining this reservation.
    pub fn executable(&self) -> PathBuf {
        self.directory().join("sugarsubstitute.exe")
    }
}
impl SelectionStore {
    /// Reserve the accepted executable without competing with an existing application run.
    /// Uses protected local selection and retained inventory, independently of repository or
    /// cache availability. The original record handle excludes replacement during verification.
    /// Run on a finite admitted native worker with `LAUNCH_RESERVATION_BYTES` available.
    /// # Errors
    /// Preserves missing/corrupt state, altered payloads, foreign protection and cancelled work.
    /// A recorded trial is returned explicitly and never interpreted as permission to launch.
    pub fn reserve_launch(
        self: &Arc<Self>,
        context: &JobContext,
    ) -> Result<ReservedLaunch, SelectionFailure> {
        self.reserve_inherited(None, context)
    }

    /// Match any inherited launch snapshot before verifying and reserving the current payload.
    ///
    /// A replacement that landed after the launcher's reservation re-resolves here when it left
    /// the startup selection unchanged (only candidate or preparation progress moved). When the
    /// accepted generation itself changed, this executable is no longer the selected application
    /// and the result is `Superseded`.
    pub(super) fn reserve_inherited(
        self: &Arc<Self>,
        inherited: Option<super::InheritedSelection>,
        context: &JobContext,
    ) -> Result<ReservedLaunch, SelectionFailure> {
        check(context)?;
        let transferred = inherited.is_some();
        let (snapshot, selection) = self.protected(context, |protection| {
            let _reader = reader(&self.root, protection, context)?;
            let current = self.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if let Some(inherited) = &inherited {
                let reserved =
                    snapshot::read(self, &inherited.file, snapshot::Purpose::Launch, protection, context)?;
                if reserved.installation() != current.installation() {
                    return Err(SelectionFailure::Changed);
                }
                if reserved != current {
                    if reserved.startup_selection() != current.startup_selection() {
                        return Err(SelectionFailure::Superseded);
                    }
                    tracing::info!(
                        operation = "installation_launch_reresolved",
                        installation = current.installation().as_str(),
                        reserved_revision = reserved.revision(),
                        current_revision = current.revision(),
                        "Launch reservation re-resolved against a newer record with the same startup selection"
                    );
                }
            }
            let selection = snapshot::create(
                self,
                protection,
                &current,
                snapshot::Purpose::Launch,
                context,
            )?;
            Ok((current, selection))
        })?;
        let generation = match snapshot.startup_selection() {
            StartupSelection::NeedsInstallation => return Ok(ReservedLaunch::NeedsInstallation),
            StartupSelection::ReconcileTrial(trial) => {
                return Ok(ReservedLaunch::ReconcileTrial(trial.clone()));
            }
            StartupSelection::Accepted(generation) => generation.clone(),
        };
        let application = VerifiedApplication::open(&self._installation, generation, context)?;
        if transferred {
            application.require_installed_launch()?;
        }
        check(context)?;
        Ok(ReservedLaunch::Application(ReservedApplication {
            application,
            _selection: selection,
            _store: Arc::clone(self),
        }))
    }
}

/// Resolve an existing installed selection for the stable launcher without creating state.
/// Offline accepted selection is sufficient. Abandoned trials are resolved without a verdict
/// under exclusive run ownership before pinning the accepted selection. No payloads or user files are retired.
/// A live trial supplies a verified invocation for coordinator forwarding without transferring
/// its run custody or claiming health. Its snapshot permits the original owner's publication.
/// # Errors
/// Preserves native ownership, record, cancellation and payload verification failures.
/// Run on a finite worker with `LAUNCH_RESERVATION_BYTES` available.
pub fn reserve_installed_launch(
    root: &Path,
    context: &JobContext,
) -> Result<ReservedLaunch, SelectionFailure> {
    match SelectionStore::existing(root, context)? {
        Some(store) => {
            let store = Arc::new(store);
            match store.load_recovered(context) {
                Ok(_) => {}
                Err(SelectionFailure::RunBusy(_)) => return store.reserve_forwarding(context),
                Err(cause) => return Err(cause),
            }
            store.reserve_launch(context)
        }
        None => Ok(ReservedLaunch::NeedsInstallation),
    }
}
