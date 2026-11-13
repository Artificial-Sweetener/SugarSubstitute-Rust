// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain authenticated application or updater helper files independently of application-run ownership.
use super::RunningApplication;
use super::application::VerifiedApplication;
use super::bundle::RuntimePackage;
use super::check;
use super::invalid;
use crate::durable_storage::files::Directory;
use std::{
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{execution::JobContext, installation::Generation};

/// Verified executable inventory for an owned helper's process-family lifetime.
/// Holds payload files and resolved ancestors against mutation/replacement, with no run,
/// selection, preparation or trust exclusion. This is neither application health nor permission
/// to activate an update. The process owner must retain it until native settlement is observed.
pub struct PreparationImage {
    source: ImageSource,
    installation: Directory,
    executable: &'static str,
}

/// Each component keeps its authoritative inventory and all verified native file pins.
enum ImageSource {
    Application(Box<VerifiedApplication>),
    Updater { generation: Generation, payload: Box<RuntimePackage> },
}

impl PreparationImage {
    /// Authenticated application or accepted updater generation supplying these files.
    pub fn generation(&self) -> &Generation {
        match &self.source {
            ImageSource::Application(application) => &application.generation,
            ImageSource::Updater { generation, .. } => generation,
        }
    }

    /// Pinned installation root for composing this installation's helper arguments.
    pub fn installation(&self) -> &Path {
        self.installation.path()
    }

    /// Pinned generation directory for the helper's working directory.
    pub fn directory(&self) -> &Path {
        match &self.source {
            ImageSource::Application(application) => application.directory.path(),
            ImageSource::Updater { payload, .. } => payload.directory(),
        }
    }

    /// Executable whose verified bytes remain pinned by this owner.
    pub fn executable(&self) -> PathBuf {
        self.directory().join(self.executable)
    }

    /// Require the launch engine role from the accepted updater's already authenticated
    /// declaration. This pure check performs no I/O and does not authorize a candidate.
    /// # Errors
    /// Rejects application-role helpers and updaters without the launch engine role.
    pub fn require_engine(&self) -> io::Result<()> {
        match &self.source {
            ImageSource::Updater { payload, .. } => {
                payload.runtime.compatibility.require_updater_engine()
            }
            ImageSource::Application(_) => {
                Err(invalid("Application helper is not an accepted update engine"))
            }
        }
    }

    /// Match the caller's actual process image to the exact pinned file, including native
    /// directory identity. Equal names or identical bytes in another directory do not qualify.
    pub(in crate::setup_storage) fn require_executable(&self, executable: &Path) -> io::Result<()> {
        let expected = self.executable();
        if executable.file_name() != expected.file_name() {
            return Err(invalid("Running updater has a different executable role"));
        }
        let parent = |path: &Path| {
            Directory::open(
                path.parent().ok_or_else(|| invalid("Updater executable parent absent"))?,
                false,
            )
        };
        if parent(executable)?.identity()? != parent(&expected)?.identity()? {
            return Err(invalid("Running updater is outside the accepted generation"));
        }
        Ok(())
    }

    /// Compose already-authenticated updater custody while its selection writer is still held.
    pub(in crate::setup_storage) fn updater(
        generation: Generation,
        payload: RuntimePackage,
        installation: Directory,
    ) -> Self {
        Self {
            source: ImageSource::Updater { generation, payload: Box::new(payload) },
            installation,
            executable: "app/sugarsubstitute-updater.exe",
        }
    }
}

impl RunningApplication {
    /// Independently retain verified files for a helper that can outlive this application's run.
    /// Reopens and verifies the exact authenticated generation while the original run still pins
    /// it. No application custody or installation lock is copied into the returned owner.
    /// Execute on an admitted worker with `super::LAUNCH_RESERVATION_BYTES` available; retain
    /// the result through the helper's observed process-family settlement.
    /// # Errors
    /// Rejects cancellation, an expired job, insufficient memory, changed payloads, unresolved
    /// native identities and filesystem/protection errors. Does not publish installation state.
    pub fn pin_preparation(&self, context: &JobContext) -> io::Result<PreparationImage> {
        check(context)?;
        let root = self
            .directory()
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| invalid("Running generation installation root is absent"))?;
        let installation = Directory::open(root, false)?;
        let application =
            VerifiedApplication::open(&installation, self.generation().clone(), context)?;
        let running = Directory::open(self.directory(), false)?;
        if running.identity()? != application.directory.identity()? {
            return Err(invalid("Helper image differs from the running generation"));
        }
        let executable = application.payload.compatibility.preparation_executable()?;
        check(context)?;
        Ok(PreparationImage {
            source: ImageSource::Application(Box::new(application)),
            installation,
            executable,
        })
    }
}
