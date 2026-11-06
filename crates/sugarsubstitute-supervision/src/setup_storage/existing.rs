// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect a selected native installation and preserve its actual saved launch inputs.
use super::RuntimePaths;
use super::Selection;
use super::check;
use super::entrypoint;
use super::invalid;
use crate::durable_storage::files::{self, Directory};
mod accepted;
mod initial;
mod recovery;
pub use recovery::RecoveryFailure;
use serde::Serialize;
use std::{borrow::Cow, fs::File, io, path::Path};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_contracts::installation::Generation;

/// Native-owned maintenance summary; presentation cannot turn it into mutation authority.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationSummary {
    /// Saved choices shown for review; maintenance re-reads authoritative state under exclusion.
    pub selection: Selection,
    /// The selected application executable is missing and needs replacement.
    pub executable_missing: bool,
    /// Saved configuration was recovered because the installation's shell entry is absent.
    pub launch_entry_missing: bool,
}

use sugarsubstitute_configuration::{
    descriptor::LaunchDescriptor, startup::MAX_STARTUP_CONFIGURATION_BYTES,
};

/// Saved configuration and launch inputs retained unchanged while a replacement is prepared.
pub(super) struct ExistingInstallation {
    pub(super) summary: InstallationSummary,
    pub(super) runtime: RuntimePaths,
    startup: Vec<u8>,
    descriptor: LaunchDescriptor,
    descriptor_bytes: Vec<u8>,
    accepted: Option<Generation>,
    _directory: Directory,
    _inputs: Vec<File>,
}

/// Inspect an installation on an admitted storage worker without executing its application.
/// # Errors
/// Rejects unknown populated folders, ambiguous/edited shell selection and damaged saved state.
/// Missing executables and shell entries are repairable from unambiguous completed settings.
/// Saved user configuration is never reset.
pub fn inspect_installation(
    path: &Path,
    context: &JobContext,
) -> io::Result<Option<InstallationSummary>> {
    check(context)?;
    files::lexical(path)?;
    if !files::exists(path, context)? {
        return Ok(None);
    }
    let root = Directory::open(path, false)?;
    if !super::ownership::present(&root)? {
        return if super::ownership::adoptable(&root, context)? {
            Ok(None)
        } else {
            Err(invalid("This folder contains files from an unknown installation"))
        };
    }
    let _memory = context.memory().claim(4 * 1024 * 1024).map_err(io::Error::other)?;
    let existing = ExistingInstallation::read(&root, context)?;
    check(context)?;
    Ok(existing.map(|existing| existing.summary))
}

impl ExistingInstallation {
    /// Bind the selected generation to this root and pin each saved input for maintenance.
    pub(super) fn read(root: &Directory, context: &JobContext) -> io::Result<Option<Self>> {
        check(context)?;
        let marker = super::ownership::read(root)?;
        if initial::incomplete_candidate(root, context)? {
            return Ok(None);
        }
        let selected = accepted::generation(root.path(), context)?;
        let existing = if let Some(generation) = &selected {
            Some(accepted::read(root, marker, generation, context)?)
        } else if let Some(store) =
            super::SelectionStore::existing(root.path(), context).map_err(io::Error::other)?
            && let Some(existing) =
                store.unaccepted_configuration(context).map_err(io::Error::other)?
        {
            Some(existing)
        } else {
            match entrypoint::selected(root, context)? {
                Some(directory) => {
                    Some(Self::from_directory(root, directory, marker, None, context)?)
                }
                None => recovery::discover(root, marker, recovery::Sources::Historical, context)?,
            }
        };
        if accepted::generation(root.path(), context)? != selected {
            return Err(invalid("Accepted installation changed during configuration inspection"));
        }
        Ok(existing)
    }

    /// Preserve agreeing protected configuration after a first trial loses its launch eligibility.
    /// The caller holds the selection writer and has validated stable shell ownership.
    pub(in crate::setup_storage) fn recover_unaccepted(
        root: &Directory,
        entry_present: bool,
        context: &JobContext,
    ) -> io::Result<Option<Self>> {
        let mut existing = recovery::discover(
            root,
            super::ownership::read(root)?,
            recovery::Sources::Protected,
            context,
        )?;
        if let Some(existing) = &mut existing {
            existing.summary.launch_entry_missing = !entry_present;
        }
        Ok(existing)
    }

    /// Read authorized prepared settings under the selection writer without asserting health.
    pub(in crate::setup_storage) fn from_prepared(
        root: &Directory,
        directory: Directory,
        generation: &Generation,
        entry_present: bool,
        context: &JobContext,
    ) -> io::Result<Self> {
        let mut existing = Self::from_directory(
            root,
            directory,
            super::ownership::read(root)?,
            Some(generation),
            context,
        )?;
        existing.summary.launch_entry_missing = !entry_present;
        Ok(existing)
    }

    /// Read saved data only; a completed setup record grants no executable or health authority.
    fn from_directory(
        root: &Directory,
        directory: Directory,
        marker: File,
        expected: Option<&Generation>,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let (record, record_file) =
            super::runtime_record::read(root, &directory, expected, context)?;
        let mut startup_file = directory.read("startup.local.toml")?;
        let startup = files::read_bounded(&mut startup_file, MAX_STARTUP_CONFIGURATION_BYTES)?;
        sugarsubstitute_configuration::initial::validate_preserved(&startup)?;
        let mut descriptor_file = directory.read("sugarsubstitute-launch.local.json")?;
        let descriptor_bytes =
            files::read_bounded(&mut descriptor_file, MAX_STARTUP_CONFIGURATION_BYTES)?;
        let descriptor = LaunchDescriptor::parse(&descriptor_bytes)?;
        descriptor.validate_maintenance_paths(files::lexical)?;
        if files::canonical_path(descriptor.maintenance_configuration()?, context)?
            != files::canonical_path(directory.path().join("startup.local.toml"), context)?
        {
            return Err(invalid(
                "Saved launch configuration is outside its installation generation",
            ));
        }
        let executable_missing = match directory.read("sugarsubstitute.exe") {
            Ok(_) => false,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => true,
            Err(cause) => return Err(cause),
        };
        Ok(Self {
            summary: InstallationSummary {
                selection: record.selection,
                executable_missing,
                launch_entry_missing: false,
            },
            runtime: record.runtime,
            startup,
            descriptor,
            descriptor_bytes,
            accepted: None,
            _directory: directory,
            _inputs: vec![marker, record_file, startup_file, descriptor_file],
        })
    }
    /// Preserve user configuration bytes and relocate only the generation-local config argument.
    pub(super) fn write_configuration(
        &self,
        destination: &Path,
        context: &JobContext,
    ) -> io::Result<()> {
        if accepted::generation(&self.summary.selection.install_root, context)? != self.accepted {
            return Err(invalid("Accepted installation changed before configuration preservation"));
        }
        let configuration = destination.join("startup.local.toml");
        files::write_immutable(&configuration, &self.startup, files::create_new)?;
        let target = Directory::open(destination, false)?;
        let descriptor = if target.identity()? == self._directory.identity()? {
            // A resumed generation already owns these validated bytes, including JSON spelling.
            Cow::Borrowed(self.descriptor_bytes.as_slice())
        } else {
            Cow::Owned(self.relocated_descriptor(destination)?.bytes()?)
        };
        files::write_immutable(
            &destination.join("sugarsubstitute-launch.local.json"),
            &descriptor,
            files::create_new,
        )
    }

    /// Pin a completed candidate's exact saved choices without writing or repairing any file.
    pub(super) fn reopen_candidate(
        &self,
        root: &Directory,
        generation: &Generation,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let path = root.path().join("releases").join(generation.id.as_str());
        let mut candidate = Self::from_directory(
            root,
            Directory::open(&path, false)?,
            super::ownership::read(root)?,
            Some(generation),
            context,
        )?;
        if candidate.summary.selection != self.summary.selection
            || candidate.runtime != self.runtime
            || candidate.startup != self.startup
            || !candidate.descriptor.same_arguments(&self.relocated_descriptor(&path)?)
            || accepted::generation(root.path(), context)? != self.accepted
        {
            return Err(invalid("Prepared candidate differs from saved installation choices"));
        }
        check(context)?;
        candidate.accepted = self.accepted.clone();
        Ok(candidate)
    }

    /// Relocate only the generation-local configuration argument for both writing and validation.
    fn relocated_descriptor(&self, destination: &Path) -> io::Result<LaunchDescriptor> {
        self.descriptor.relocated(&destination.join("startup.local.toml"))
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) mod fixture;
