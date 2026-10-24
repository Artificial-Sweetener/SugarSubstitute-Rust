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
use serde::{Deserialize, Serialize};
use std::{fs::File, io, path::Path};
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadyRecord {
    schema_version: u32,
    bundle_sha256: String,
    selection: Selection,
    runtime: RuntimePaths,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LaunchDescriptor {
    schema_version: u32,
    arguments: Vec<String>,
}

/// Saved configuration and launch inputs retained unchanged while a replacement is prepared.
pub(super) struct ExistingInstallation {
    pub(super) summary: InstallationSummary,
    pub(super) runtime: RuntimePaths,
    startup: Vec<u8>,
    descriptor: LaunchDescriptor,
    configuration_index: usize,
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
    if !files::exists(root.path().join(".substitute-installation.json"), context)? {
        return if root.entries(context)?.next().transpose()?.is_none() {
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
        } else {
            match entrypoint::selected(root, context)? {
                Some(directory) => {
                    Some(Self::from_directory(root, directory, marker, None, context)?)
                }
                None => recovery::discover(root, marker, context)?,
            }
        };
        if accepted::generation(root.path(), context)? != selected {
            return Err(invalid("Accepted installation changed during configuration inspection"));
        }
        Ok(existing)
    }

    /// Read saved data only; a completed setup record grants no executable or health authority.
    fn from_directory(
        root: &Directory,
        directory: Directory,
        marker: File,
        accepted: Option<&Generation>,
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        let mut record_file = directory.read("installation-ready.json")?;
        let record: ReadyRecord =
            serde_json::from_slice(&files::read_bounded(&mut record_file, 1024 * 1024)?)
                .map_err(io::Error::other)?;
        if record.schema_version != 1
            || record.bundle_sha256.len() != 64
            || !record.bundle_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid("Unsupported native installation record"));
        }
        if accepted.is_some_and(|generation| generation.package.as_str() != record.bundle_sha256) {
            return Err(invalid("Saved configuration differs from the accepted package identity"));
        }
        record.selection.validate()?;
        if crate::durable_storage::files::canonical_path(&record.selection.install_root, context)?
            != crate::durable_storage::files::canonical_path(root.path(), context)?
        {
            return Err(invalid("Saved installation choices belong to another folder"));
        }
        for path in [
            &record.runtime.directory,
            &record.runtime.python,
            &record.runtime.frontend,
            &record.runtime.metadata,
        ] {
            files::lexical(path)?;
        }
        let mut startup_file = directory.read("startup.local.toml")?;
        let startup = files::read_bounded(&mut startup_file, 65536)?;
        let value: toml::Value = std::str::from_utf8(&startup)
            .map_err(io::Error::other)?
            .parse()
            .map_err(io::Error::other)?;
        if value.get("schema_version").and_then(toml::Value::as_integer) != Some(1) {
            return Err(invalid("Saved startup configuration requires a compatible installer"));
        }
        let mut descriptor_file = directory.read("sugarsubstitute-launch.local.json")?;
        let descriptor: LaunchDescriptor =
            serde_json::from_slice(&files::read_bounded(&mut descriptor_file, 65536)?)
                .map_err(io::Error::other)?;
        let configuration_index = descriptor.validate(&directory, context)?;
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
            configuration_index,
            accepted: accepted.cloned(),
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
        files::write_immutable(
            &destination.join("sugarsubstitute-launch.local.json"),
            &serde_json::to_vec(&self.relocated_descriptor(destination)?)
                .map_err(io::Error::other)?,
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
        let candidate = Self::from_directory(
            root,
            Directory::open(&path, false)?,
            super::ownership::read(root)?,
            Some(generation),
            context,
        )?;
        if candidate.summary.selection != self.summary.selection
            || candidate.runtime != self.runtime
            || candidate.startup != self.startup
            || candidate.descriptor.arguments != self.relocated_descriptor(&path)?.arguments
            || accepted::generation(root.path(), context)? != self.accepted
        {
            return Err(invalid("Prepared candidate differs from saved installation choices"));
        }
        check(context)?;
        Ok(candidate)
    }

    /// Relocate only the generation-local configuration argument for both writing and validation.
    fn relocated_descriptor(&self, destination: &Path) -> io::Result<LaunchDescriptor> {
        let mut descriptor =
            LaunchDescriptor { schema_version: 1, arguments: self.descriptor.arguments.clone() };
        descriptor.arguments[self.configuration_index] = destination
            .join("startup.local.toml")
            .to_str()
            .ok_or_else(|| invalid("Installed configuration path is not Unicode"))?
            .to_owned();
        Ok(descriptor)
    }
}
impl LaunchDescriptor {
    /// Check the supported argument contract and return its configuration-value index.
    fn validate(&self, directory: &Directory, context: &JobContext) -> io::Result<usize> {
        let expected = [
            "--startup-config",
            "--frontend-package",
            "--frontend-metadata",
            "--sugarcubes-root",
            "--backend-script",
            "--profile-directory",
        ];
        if self.schema_version != 1 || self.arguments.len() != expected.len() * 2 {
            return Err(invalid("Saved launch descriptor requires a compatible installer"));
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut configuration = None;
        for (index, pair) in self.arguments.chunks_exact(2).enumerate() {
            if !expected.contains(&pair[0].as_str())
                || !seen.insert(&pair[0])
                || pair[1].len() > 4096
            {
                return Err(invalid("Saved launch descriptor contains unsupported options"));
            }
            files::lexical(Path::new(&pair[1]))?;
            if pair[0] == "--startup-config" {
                if crate::durable_storage::files::canonical_path(&pair[1], context)?
                    != crate::durable_storage::files::canonical_path(
                        directory.path().join("startup.local.toml"),
                        context,
                    )?
                {
                    return Err(invalid(
                        "Saved launch configuration is outside its installation generation",
                    ));
                }
                configuration = Some(index * 2 + 1);
            }
        }
        configuration.ok_or_else(|| invalid("Saved launch configuration is absent"))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) mod fixture;
