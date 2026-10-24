// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protected selection publication and independent OS-backed application-run ownership.
//!
//! Storage operations run on the execution owner's worker. A selection proposal proves
//! neither package trust nor process health. Unpublished staging is never recovery authority.
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
pub(super) mod publication;
mod reservation;
pub use reservation::{ReservedApplication, ReservedLaunch, reserve_installed_launch};
mod run;
use super::check;
use crate::durable_storage::{
    files::{self, Directory},
    protected::directories::Mode,
    protection::{self, Protection},
    staging,
};
pub use failure::SelectionFailure;
pub(crate) use run::UnstartedTrial;
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

const DIRECTORY: &str = ".selection-state";
const RECORD: &str = ".substitute-selection.json";
const LOCK: &str = ".selection-lock";
pub(super) const NAMESPACE_MARKER: &str = ".selection-enabled.json";

/// Pinned native installation and protected selection namespace.
/// Short writer ownership is independent of application lifetime and preparation workers.
pub struct SelectionStore {
    root: Directory,
    _installation: Directory,
    _ownership: File,
    _namespace_marker: Option<File>,
    _memory: MemoryClaim,
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

    /// Load a complete record; absence after opening is damaged state, never bootstrap authority.
    /// # Errors
    /// Returns busy without waiting on a writer and preserves invalid record bytes.
    pub fn load(
        &self,
        context: &JobContext,
    ) -> Result<Option<InstallationRecord>, SelectionFailure> {
        self.protected(context, |protection| {
            let _writer = writer(&self.root, protection, context)?;
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
            || expected.rejected() != replacement.rejected()
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

    fn read(
        &self,
        protection: &Protection,
    ) -> Result<Option<InstallationRecord>, SelectionFailure> {
        Ok(self.read_pinned(protection)?.map(|(record, _pin)| record))
    }

    /// Validate and retain the same protected record; its sharing mode excludes atomic replacement.
    fn read_pinned(
        &self,
        protection: &Protection,
    ) -> Result<Option<(InstallationRecord, File)>, SelectionFailure> {
        let mut file = match self.root.read(RECORD) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(cause.into()),
        };
        protection.verify_file(&file)?;
        let bytes = files::read_bounded(&mut file, MAX_RECORD_BYTES)?;
        Ok(Some((InstallationRecord::decode(&bytes)?, file)))
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
fn writer(
    root: &Directory,
    protection: &Protection,
    context: &JobContext,
) -> Result<File, SelectionFailure> {
    let lock = protection.rendezvous(&root.path().join(LOCK), context)?;
    lock.try_lock().map_err(|cause| match cause {
        std::fs::TryLockError::WouldBlock => SelectionFailure::WriterBusy,
        std::fs::TryLockError::Error(cause) => SelectionFailure::Storage(cause),
    })?;
    Ok(lock)
}

#[cfg(test)]
mod tests;
