// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read application entry compatibility only from payload bytes already hashed and pinned.
use std::{
    fs::File,
    io::{self, Seek, SeekFrom},
};
use sugarsubstitute_contracts::application_launch::{
    ApplicationContract, CONTRACT_FILE, CURRENT_LAUNCHER_CONTRACT, LAUNCHER_CONTRACT_FILE,
    LauncherContract, MAX_CONTRACT_BYTES, Rejection,
    installation::{self, InstallationActivationContract},
    updater::{PREPARATION_PROTOCOL, UPDATER_CONTRACT_FILE, UpdaterContract},
};

/// Retain an authenticated application declaration while preserving older package readability.
#[derive(Default)]
pub(super) struct Compatibility {
    application: Option<ApplicationContract>,
    launcher: Option<LauncherContract>,
    updater: Option<Box<UpdaterContract>>,
    installation: Option<InstallationActivationContract>,
}
impl Compatibility {
    /// Called only after this exact retained file matches its authenticated inventory entry.
    pub(super) fn inspect_verified(&mut self, path: &str, file: &mut File) -> io::Result<()> {
        let Some(name) = path.strip_prefix("app/") else { return Ok(()) };
        if ![
            CONTRACT_FILE,
            LAUNCHER_CONTRACT_FILE,
            UPDATER_CONTRACT_FILE,
            installation::CONTRACT_FILE,
        ]
        .contains(&name)
        {
            return Ok(());
        }
        file.seek(SeekFrom::Start(0))?;
        let bytes = crate::durable_storage::files::read_bounded(file, MAX_CONTRACT_BYTES)?;
        if name == CONTRACT_FILE {
            self.application = Some(ApplicationContract::decode(&bytes).map_err(io::Error::other)?);
        } else if name == LAUNCHER_CONTRACT_FILE {
            self.launcher = Some(LauncherContract::decode(&bytes).map_err(io::Error::other)?);
        } else if name == installation::CONTRACT_FILE {
            self.installation =
                Some(InstallationActivationContract::decode(&bytes).map_err(io::Error::other)?);
        } else {
            self.updater =
                Some(Box::new(UpdaterContract::decode(&bytes).map_err(io::Error::other)?));
        }
        Ok(())
    }

    /// Packages carrying a launcher must pair it with an application it can actually start.
    /// This engine reads the current selection schema; a foreign reader needs an engine update.
    /// Historical distributions without a launcher remain readable for explicit maintenance.
    pub(super) fn validate_pair(&self) -> io::Result<()> {
        if let Some(launcher) = &self.launcher {
            launcher
                .require_application(
                    self.application()?,
                    sugarsubstitute_contracts::installation::SCHEMA_VERSION,
                )
                .map_err(io::Error::other)?;
        }
        Ok(())
    }

    /// Old packages remain inspectable; explicit inherited entry requires advertised support.
    pub(super) fn require_installed_launch(&self) -> io::Result<()> {
        self.require_launcher(
            &LauncherContract::decode(CURRENT_LAUNCHER_CONTRACT).map_err(io::Error::other)?,
        )
    }

    /// Require the receiver's actual two-handle trial entry from authenticated package metadata.
    pub(super) fn require_trial_launch(&self) -> io::Result<()> {
        self.application()?
            .require_installed_launch(
                sugarsubstitute_contracts::application_launch::TRIAL_LAUNCH_PROTOCOL,
            )
            .map_err(io::Error::other)
    }

    /// Whether this authenticated distribution includes its independently declared launcher.
    pub(super) fn has_launcher(&self) -> bool {
        self.launcher.is_some()
    }

    /// Explicit completion installs a launcher capable of reopening this exact durable choice.
    /// Historical accepted-only launchers remain readable but cannot complete this setup flow.
    pub(super) fn require_installation_activation(
        &self,
        minimum_engine_release: u64,
    ) -> io::Result<()> {
        self.require_launcher(
            self.launcher.as_ref().ok_or_else(|| io::Error::other(Rejection::Missing))?,
        )?;
        self.require_trial_launch()?;
        self.installation
            .as_ref()
            .ok_or_else(|| io::Error::other(Rejection::Missing))?
            .require_installation(
                installation::AUTHORIZATION_SCHEMA,
                minimum_engine_release,
                sugarsubstitute_update::ENGINE_PROTOCOL,
            )
            .map_err(io::Error::other)
    }

    /// Select only authenticated updater roles; older distributions use their application role.
    /// A present incompatible declaration is an error, never permission to silently fall back.
    pub(super) fn preparation_executable(&self) -> io::Result<&'static str> {
        if self.updater.is_some() {
            self.require_updater()?;
            Ok("sugarsubstitute-updater.exe")
        } else {
            Ok("sugarsubstitute.exe")
        }
    }

    /// Independent updater packages must explicitly support this caller's process entry.
    pub(super) fn require_updater(&self) -> io::Result<()> {
        self.updater
            .as_ref()
            .ok_or_else(|| io::Error::other(Rejection::Missing))?
            .require_preparation(
                PREPARATION_PROTOCOL,
                sugarsubstitute_contracts::installation::SCHEMA_VERSION,
            )
            .map_err(io::Error::other)
    }

    /// A pending engine trial requires the probe actually implemented by its authenticated image.
    /// Historical preparation-only images remain inspectable but cannot enter a probe trial.
    pub(super) fn require_updater_probe(&self) -> io::Result<()> {
        self.updater
            .as_ref()
            .ok_or_else(|| io::Error::other(Rejection::Missing))?
            .require_probe(
                sugarsubstitute_contracts::application_launch::updater::probe::PROBE_PROTOCOL,
                sugarsubstitute_contracts::installation::SCHEMA_VERSION,
            )
            .map_err(io::Error::other)
    }

    /// Startup delegation requires the independently implemented activation entry.
    pub(super) fn require_updater_activation(&self) -> io::Result<()> {
        self.updater
            .as_ref()
            .ok_or_else(|| io::Error::other(Rejection::Missing))?
            .require_activation(
                sugarsubstitute_contracts::application_launch::updater::ACTIVATION_PROTOCOL,
                sugarsubstitute_contracts::installation::SCHEMA_VERSION,
            )
            .map_err(io::Error::other)
    }

    /// Check the installed launcher's declaration against this exact application's entry.
    pub(super) fn require_launcher(&self, launcher: &LauncherContract) -> io::Result<()> {
        launcher
            .require_application(
                self.application()?,
                sugarsubstitute_contracts::installation::SCHEMA_VERSION,
            )
            .map_err(io::Error::other)
    }

    fn application(&self) -> io::Result<&ApplicationContract> {
        self.application.as_ref().ok_or_else(|| io::Error::other(Rejection::Missing))
    }
}
