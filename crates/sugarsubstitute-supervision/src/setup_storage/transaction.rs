// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep desktop generations separate from selected ComfyUI environments and authored data.
mod activation;
pub(in crate::setup_storage) mod admission;
mod candidate;
pub(in crate::setup_storage) mod generation;
mod records;
mod reservation;
mod resume;
mod session;
pub use session::InstallationSession;
mod trial;
use super::VerifiedBundle;
use super::check;
use super::existing::ExistingInstallation;
use super::invalid;
use crate::durable_storage::files::{self, Directory};
use admission::Admission;
pub use candidate::PreparedCandidate;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::execution::JobContext;
pub use trial::AdmittedCandidate;

/// User choices carry paths and issued selections, never executable URLs or commands.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    /// Supported product locale.
    pub locale: String,
    /// Desktop installation root.
    pub install_root: PathBuf,
    /// `managed` creates an environment; `attached` checks an existing environment.
    pub mode: String,
    /// Standalone parent for provisioning, or Comfy source for attachment.
    pub comfy_root: PathBuf,
    /// Optional explicit interpreter for an attached environment.
    pub python: String,
    /// Model library folder; files remain in place.
    pub models: PathBuf,
    /// Output folder; files remain in place.
    pub outputs: PathBuf,
    /// Opaque capability issued by the provisioning worker.
    #[serde(default)]
    pub selection_token: Option<String>,
}
impl Selection {
    /// Validate choices before filesystem admission.
    /// # Errors
    /// Rejects unsupported locales, modes, relative paths and missing selections.
    pub fn validate(&self) -> io::Result<()> {
        for path in [&self.install_root, &self.comfy_root, &self.models, &self.outputs] {
            files::lexical(path)?;
        }
        if !sugarsubstitute_contracts::localization::LOCALES
            .iter()
            .any(|locale| locale.id == self.locale)
            || !matches!(self.mode.as_str(), "managed" | "attached")
            || (self.mode == "managed"
                && self
                    .selection_token
                    .as_ref()
                    .is_none_or(|token| token.is_empty() || token.len() > 80))
        {
            return Err(invalid("Review the installation choices"));
        }
        if !self.python.is_empty() {
            files::lexical(Path::new(&self.python))?;
        }
        Ok(())
    }
}

/// Paths returned by the selected environment's actual Python.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePaths {
    /// Comfy source directory containing main.py.
    pub directory: PathBuf,
    /// Environment interpreter.
    pub python: PathBuf,
    /// Installed comfyui_frontend_package directory.
    pub frontend: PathBuf,
    /// Corresponding distribution METADATA file.
    pub metadata: PathBuf,
}

/// Retain installation custody until provisioning and activation settle.
pub struct PreparedInstallation {
    session: InstallationSession,
    generation: PathBuf,
    bundle: Arc<VerifiedBundle>,
    _generation: Directory,
    _generation_marker: File,
}

/// A fully published launch location ready for an explicit user-requested startup.
pub struct InstalledApplication {
    /// Absolute stable launcher or directly published desktop executable.
    pub executable: PathBuf,
    /// Working directory belonging to this executable.
    pub directory: PathBuf,
}

impl PreparedInstallation {
    /// Return the saved environment only for maintenance; no provisioning worker is needed.
    pub fn maintenance_runtime(&self) -> Option<&RuntimePaths> {
        self.session.existing.as_ref().map(|existing| &existing.runtime)
    }

    /// Copy desktop payloads in a separately cancellable job without a total copy deadline.
    /// # Errors
    /// Preserves partial files and the held installation lock on failed or cancelled copies.
    pub fn stage(&self, context: &JobContext) -> io::Result<()> {
        crate::durable_storage::protection::with(context, |protection| {
            self.bundle
                .payload
                .copy("app/", &self.generation, context, |path| protection.file(path, context))
        })
    }

    /// Finish immutable generation files without changing any launch selection.
    fn complete(&self, runtime: &RuntimePaths, context: &JobContext) -> io::Result<()> {
        check(context)?;
        self.bundle.retain_inventory(&self.generation, context)?;
        if let Some(existing) = &self.session.existing {
            if runtime != &existing.runtime {
                return Err(invalid("Maintenance runtime differs from the saved installation"));
            }
            existing.write_configuration(&self.generation, context)?;
        } else {
            self.configure_new(runtime, context)?;
        }
        if self.bundle.download_identity().is_none() {
            super::entrypoint::stage(&self.generation)?;
        }
        check(context)?;
        files::write_immutable(
            &self.generation.join("installation-ready.json"),
            &serde_json::to_vec(&serde_json::json!({
                "schema_version": 1, "bundle_sha256": self.bundle.digest(),
                "selection": self.session.selection, "runtime": runtime,
            }))
            .map_err(io::Error::other)?,
            files::create_new,
        )?;
        Ok(())
    }
    /// Integrate the selected environment and write initial configuration for a fresh install.
    fn configure_new(&self, runtime: &RuntimePaths, context: &JobContext) -> io::Result<()> {
        let _comfy = Directory::open(&runtime.directory, false)?;
        for path in [&runtime.python, &runtime.metadata] {
            files::lexical(path)?;
            let parent = Directory::open(
                path.parent().ok_or_else(|| invalid("Runtime parent absent"))?,
                false,
            )?;
            let leaf = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| invalid("Runtime leaf absent"))?;
            let _input = parent.read(leaf)?;
        }
        let _frontend = Directory::open(&runtime.frontend, false)?;
        let nodes = Directory::open(&runtime.directory.join("custom_nodes"), true)?;
        let cube = nodes.path().join("SugarCubes");
        if !cube.try_exists()? {
            let stage =
                crate::durable_storage::staging::directory(&_comfy, ".substitute-node-stage")?;
            self.bundle.payload.copy("sugarcubes/", &stage, context, files::create_new)?;
            {
                let directory = Directory::open(&stage, false)?;
                let project = files::read_bounded(&mut directory.read("pyproject.toml")?, 65536)?;
                crate::installed_frontend::validate_cube_metadata(&project)
                    .map_err(io::Error::other)?;
            }
            check(context)?;
            crate::durable_storage::staging::publish(&stage, &cube)?;
        }
        let cube_directory = Directory::open(&cube, false)?;
        let project = files::read_bounded(&mut cube_directory.read("pyproject.toml")?, 65536)?;
        crate::installed_frontend::validate_cube_metadata(&project).map_err(io::Error::other)?;
        let _models = Directory::open(&self.session.selection.models, true)?;
        let _outputs = Directory::open(&self.session.selection.outputs, true)?;
        check(context)?;
        records::write(&self.generation, &self.session.selection, runtime, &cube)?;
        Ok(())
    }
}
