// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protected selection publication and independent OS-backed application-run ownership.
//!
//! Storage operations run on the execution owner's worker. A selection proposal proves
//! neither package trust nor process health. Unpublished staging is never recovery authority.
mod clock;
#[allow(unsafe_code)]
mod entry;
mod failure;
pub use entry::{InheritedLaunch, capture_launch};
mod completion;
mod configuration;
#[allow(unsafe_code)]
mod inheritance;
mod initialize;
mod launcher;
pub(in crate::setup_storage) use completion::InstallationAuthorization;
pub use inheritance::{InheritedSelection, LaunchInheritance, capture_launch_selection};
mod namespace;
mod presentation;
pub use presentation::{
    InstalledRelease, PreparedUpdate, Rollback, UpdateSelection, read_update_selection,
};
pub(super) mod publication;
mod reservation;
mod route;
pub use route::{ROUTE_RESERVATION_BYTES, Route, RouteFailure, route_launch};
mod snapshot;
mod withholding;
pub use reservation::{ReservedApplication, ReservedLaunch, reserve_installed_launch};
pub use withholding::release_withheld;
mod run;
use super::check;
use super::short_lock::{self, LockKind};
use crate::durable_storage::{
    files::{self, Directory},
    protected::directories::Mode,
    protection::{self, Protection},
    staging,
};
pub(crate) use clock::StrikeClock;
pub use failure::SelectionFailure;
pub(crate) use run::UnstartedTrial;
#[cfg(test)]
pub(in crate::setup_storage) use run::unix_now;
pub use run::{AcceptedApplication, InstallationLaunch, InstallationRun, OwnedTrial};
pub use run::{
    InheritedTrial, TRIAL_WITNESS_RESERVATION_BYTES, TrialApplication, TrialTransfer, TrialWitness,
    TrialWitnessTransfer, capture_trial_custody,
};
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};
use sugarsubstitute_contracts::{
    execution::{JobContext, MemoryClaim},
    installation::{Candidate, InstallationRecord, MAX_RECORD_BYTES},
};

/// Protected selection namespace inside the installation state folder.
pub(super) const DIRECTORY: &str = "selection";
const RECORD: &str = ".substitute-selection.json";
const LOCK: &str = ".selection-lock";
pub(super) const NAMESPACE_MARKER: &str = "selection-enabled.json";
/// Rendezvous serializing first selection enablement with earlier installer publication.
pub(super) const TRANSITION_LOCK: &str = "selection-transition-lock";

/// Pinned native installation and protected selection namespace.
/// Short writer ownership is independent of application lifetime and preparation workers.
pub struct SelectionStore {
    root: Directory,
    _installation: Directory,
    _ownership: File,
    _namespace_marker: Option<File>,
    _memory: MemoryClaim,
    clock: StrikeClock,
}
impl SelectionStore {
    /// Open an initialized owned namespace and enable continuity for an earlier valid record.
    /// No accepted generation or trust is inferred from a previously installed executable.
    /// # Errors
    /// Rejects absent/incomplete state, links, foreign permissions and invalid markers.
    /// Earlier state is preserved for explicit verified installer recovery.
    pub fn open(root: &Path, context: &JobContext) -> Result<Self, SelectionFailure> {
        Self::open_namespace(root, context, true)?.ok_or(SelectionFailure::Missing)
    }

    /// Date strikes and decide withholding with `clock` instead of the system clock.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_strike_clock(mut self, clock: StrikeClock) -> Self {
        self.clock = clock;
        self
    }

    /// The clock this store dates strikes with and checks withholding against.
    pub(in crate::setup_storage) fn strike_clock(&self) -> StrikeClock {
        self.clock
    }

    /// Inspect an existing namespace without creating one for an earlier installation.
    /// Absence is allowed only before selection has been enabled for this installation.
    pub(super) fn existing(
        root: &Path,
        context: &JobContext,
    ) -> Result<Option<Self>, SelectionFailure> {
        Self::open_namespace(root, context, false)
    }

    /// Pin existing ancestry and validate complete state before enabling its continuity marker.
    fn open_namespace(
        root: &Path,
        context: &JobContext,
        enable_continuity: bool,
    ) -> Result<Option<Self>, SelectionFailure> {
        let (installation, ownership, memory) = Self::installation(root, context)?;
        let Some((root, marker)) = namespace::open(&installation, enable_continuity, context)?
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            root,
            _installation: installation,
            _ownership: ownership,
            _namespace_marker: marker,
            _memory: memory,
            clock: StrikeClock::System,
        }))
    }

    /// Pin installation identity before opening or initializing its protected selection.
    fn installation(
        root: &Path,
        context: &JobContext,
    ) -> Result<(Directory, File, MemoryClaim), SelectionFailure> {
        check(context)?;
        let memory = context.memory().claim(1024 * 1024).map_err(io::Error::other)?;
        let installation = Directory::open(root, false)?;
        let ownership = super::ownership::read(&installation)?;
        for earlier in [RECORD, LOCK] {
            match crate::durable_storage::files::inspect(installation.path().join(earlier), context)
            {
                Ok(_) => {
                    return Err(
                        super::invalid("Unprotected selection requires verified recovery").into()
                    );
                }
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
                Err(cause) => return Err(cause.into()),
            }
        }
        Ok((installation, ownership, memory))
    }

    /// Load a complete record under a shared lock; concurrent readers proceed together.
    /// Absence after opening is damaged state, never bootstrap authority.
    /// # Errors
    /// Waits out an in-flight publication, then returns `LockTimeout` or `LockCancelled`;
    /// preserves invalid record bytes.
    pub fn load(
        &self,
        context: &JobContext,
    ) -> Result<Option<InstallationRecord>, SelectionFailure> {
        self.protected(context, |protection| {
            let _reader = reader(&self.root, protection, context)?;
            let record = self.read(protection)?;
            check(context)?;
            Ok(record)
        })
    }

    /// Publish preparation progress without changing running or accepted generation authority.
    /// Compare the entire record and its revision under independent writer exclusion.
    /// # Errors
    /// Rejects stale proposals, activation changes without run ownership and invalid storage.
    pub fn compare_exchange(
        &self,
        expected: &InstallationRecord,
        replacement: &InstallationRecord,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        if expected.accepted() != replacement.accepted()
            || expected.retained() != replacement.retained()
            || expected.strikes() != replacement.strikes()
            || expected.rollback() != replacement.rollback()
            || expected.repair() != replacement.repair()
            || matches!(expected.candidate(), Some(Candidate::Trial { .. }))
            || matches!(replacement.candidate(), Some(Candidate::Trial { .. }))
        {
            return Err(SelectionFailure::RunOwnership);
        }
        self.replace(expected, replacement, context)
    }

    /// Commit point shared by preparation and the separately owned activation path.
    /// Cancellation after publication must not misreport a committed transition as unwritten.
    fn replace(
        &self,
        expected: &InstallationRecord,
        replacement: &InstallationRecord,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        check(context)?;
        if expected.installation() != replacement.installation()
            || expected.revision().checked_add(1) != Some(replacement.revision())
        {
            return Err(SelectionFailure::Revision);
        }
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
            if self.read(protection)?.as_ref() != Some(expected) {
                return Err(SelectionFailure::Changed);
            }
            self.publish(protection, replacement, context)
        })
    }

    /// Verify parent protection on each operation and preserve both failure outcomes.
    fn protected<T>(
        &self,
        context: &JobContext,
        operation: impl FnOnce(&mut Protection) -> Result<T, SelectionFailure>,
    ) -> Result<T, SelectionFailure> {
        protection::with_domain(context, |protection| {
            protection.directory(self.root.path(), Mode::Existing, context)?;
            operation(protection)
        })
        .map_err(SelectionFailure::from)
    }

    /// Validate and decode the protected record. The caller holds the shared or exclusive lock,
    /// so no publication replaces it during the read; the handle closes before returning.
    fn read(
        &self,
        protection: &Protection,
    ) -> Result<Option<InstallationRecord>, SelectionFailure> {
        let mut file = match self.root.read(RECORD) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(cause.into()),
        };
        protection.verify_file(&file)?;
        let bytes = files::read_bounded(&mut file, MAX_RECORD_BYTES)?;
        Ok(Some(InstallationRecord::decode(&bytes)?))
    }

    /// Keep revision-zero selection stable while its trust store is initialized.
    pub(super) fn lock_initial(
        &self,
        context: &JobContext,
    ) -> Result<(InstallationRecord, File), SelectionFailure> {
        self.protected(context, |protection| {
            let writer = writer(&self.root, protection, context)?;
            let record = self.read(protection)?.ok_or(SelectionFailure::Missing)?;
            if record.revision() != 0 {
                return Err(SelectionFailure::Revision);
            }
            Ok((record, writer))
        })
    }

    fn publish(
        &self,
        protection: &Protection,
        record: &InstallationRecord,
        context: &JobContext,
    ) -> Result<(), SelectionFailure> {
        let bytes = record.encode()?;
        let staged = staging::unique_path(&self.root, ".selection-stage")?;
        let mut file = protection.file(&staged, context)?;
        file.write_all(&bytes)?;
        files::sync(&file, context)?;
        drop(file);
        check(context)?;
        staging::replace_record(&staged, &self.root.path().join(RECORD))?;
        Ok(())
    }
}

/// Serialize selection publication and its one-time continuity marker with the same OS lock.
/// The exclusive lock waits for in-flight readers and other writers within the bounded wait.
fn writer(
    root: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> Result<File, SelectionFailure> {
    locked(root, protection, LockKind::Exclusive, context)
}

/// Hold the shared lock for one consistent read of the record and any files written with it.
/// Readers run together and wait only for an in-flight publication.
fn reader(
    root: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> Result<File, SelectionFailure> {
    locked(root, protection, LockKind::Shared, context)
}

fn locked(
    root: &Directory,
    protection: &Protection,
    kind: LockKind,
    context: &JobContext,
) -> Result<File, SelectionFailure> {
    let lock = protection.waitable_rendezvous(&root.path().join(LOCK), context)?;
    short_lock::acquire(&lock, kind, context)?;
    Ok(lock)
}

#[cfg(test)]
mod tests;
