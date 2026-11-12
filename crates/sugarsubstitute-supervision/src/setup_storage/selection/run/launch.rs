// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Open a healthy accepted generation from protected installation state without network or cache.
pub(super) mod entrypoint;
use super::*;
use crate::setup_storage::{ReservedApplication, ReservedLaunch};
use crate::setup_storage::{application::VerifiedApplication, check};
use std::path::{Path, PathBuf};
use sugarsubstitute_contracts::installation::{Generation, StartupSelection};

/// The elected coordinator's next action under actual installation-run ownership.
pub enum InstallationLaunch {
    /// Verified accepted application with live installation exclusion retained.
    Application(Box<AcceptedApplication>),
    /// No accepted generation exists; installation or repair must finish first.
    NeedsInstallation,
}

/// Accepted application bytes pinned for launch and its entire owned process-family lifetime.
/// Retains run exclusion if the original run handle is dropped. This is not a process-settlement
/// receipt; the host must retain it until native custody confirms the family has finished.
pub struct AcceptedApplication {
    application: VerifiedApplication,
    custody: Arc<Custody>,
    installation: sugarsubstitute_contracts::installation::InstallationId,
}
impl AcceptedApplication {
    /// Borrow the existing selection's stable installation identity while its native run is retained.
    pub fn installation_id(&self) -> &sugarsubstitute_contracts::installation::InstallationId {
        &self.installation
    }
    /// Durable accepted identity, including the original authenticated package manifest digest.
    pub fn generation(&self) -> &Generation {
        &self.application.generation
    }

    /// Verified generation's absolute directory for native launch composition.
    pub fn directory(&self) -> &Path {
        self.application.directory.path()
    }

    /// Absolute executable whose bytes remain held against replacement or mutation.
    pub fn executable(&self) -> PathBuf {
        self.directory().join("sugarsubstitute.exe")
    }
}

impl InstallationRun {
    /// Resolve the accepted generation while preserving independent background preparation.
    /// Uses only protected installation state and retained inventory: network reachability,
    /// repository expiry and download-cache retention do not gate an already accepted release.
    /// One run admits either this application or one trial, never both.
    /// An inherited launch snapshot must still select this executable's accepted generation.
    /// # Errors
    /// Rejects corrupt/missing inventory, altered payloads, links and unknown protection; a changed
    /// accepted generation is `Superseded`. Interrupted trials are resolved without a verdict
    /// under this fresh run's custody; recovery never accepts the candidate or grants process-settlement or
    /// cleanup authority.
    /// Execute on a finite admitted native worker with `LAUNCH_RESERVATION_BYTES` available.
    pub fn select_launch(
        &mut self,
        inherited: Option<super::super::InheritedSelection>,
        context: &JobContext,
    ) -> Result<InstallationLaunch, SelectionFailure> {
        check(context)?;
        if self.issued {
            return Err(SelectionFailure::RunOwnership);
        }
        let mut reserved = self.custody.store.reserve_inherited(inherited, context)?;
        if matches!(reserved, ReservedLaunch::ReconcileTrial(_)) {
            self.recover_abandoned_trial(context)?;
            reserved = self.custody.store.reserve_launch(context)?;
        }
        match reserved {
            ReservedLaunch::NeedsInstallation => Ok(InstallationLaunch::NeedsInstallation),
            ReservedLaunch::ReconcileTrial(_) => Err(SelectionFailure::Changed),
            ReservedLaunch::Application(reservation) => self
                .adopt_reserved(reservation, context)
                .map(|application| InstallationLaunch::Application(Box::new(application))),
        }
    }

    /// Transfer a verified launch reservation into this elected coordinator's live run custody.
    /// The accepted generation is rechecked against the current record before admission.
    /// Payloads are moved, not reopened or rehashed. This admits an already accepted application;
    /// it neither accepts a trial nor proves that a previous process family has finished.
    /// # Errors
    /// Rejects another native installation, repeated admission, stale state or cancellation.
    /// Execute on a finite native worker while retaining the reservation and run owner.
    pub fn adopt_reserved(
        &mut self,
        reservation: ReservedApplication,
        context: &JobContext,
    ) -> Result<AcceptedApplication, SelectionFailure> {
        check(context)?;
        if self.issued {
            return Err(SelectionFailure::RunOwnership);
        }
        self.validate_installation(&reservation._store._installation)?;
        let current = self.custody.store.load(context)?.ok_or(SelectionFailure::Missing)?;
        if !matches!(current.startup_selection(), StartupSelection::Accepted(generation) if generation == reservation.generation())
        {
            return Err(SelectionFailure::Superseded);
        }
        check(context)?;
        self.issued = true;
        Ok(AcceptedApplication {
            application: reservation.application,
            custody: Arc::clone(&self.custody),
            installation: current.installation().clone(),
        })
    }
}
