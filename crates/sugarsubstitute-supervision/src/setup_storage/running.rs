// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the elected application's generation to protected installation run ownership.
use super::AcceptedApplication;
use super::InstallationLaunch;
use super::InstallationRun;
use super::SelectionStore;
use super::check;
use super::invalid;
use super::{InheritedLaunch, TrialApplication};
use crate::durable_storage::files::Directory;
use std::{io, path::Path, sync::Arc};
use sugarsubstitute_contracts::execution::JobContext;

/// Retained installation ownership for the elected coordinator, through its full teardown.
pub struct RunningApplication {
    application: Application,
    backend: super::backend_data::BackendDataAuthority,
}
enum Application {
    Accepted(Box<AcceptedApplication>),
    Trial(Box<TrialApplication>),
}
impl RunningApplication {
    /// Exact verified generation retained by this coordinator.
    pub fn generation(&self) -> &sugarsubstitute_contracts::installation::Generation {
        match &self.application {
            Application::Accepted(application) => application.generation(),
            Application::Trial(application) => &application.attempt().generation,
        }
    }
    /// Verified running directory whose payload remains protected by this owner.
    pub fn directory(&self) -> &Path {
        match &self.application {
            Application::Accepted(application) => application.directory(),
            Application::Trial(application) => application.directory(),
        }
    }
    /// Borrow live trial authority only while this running generation is a trial.
    pub fn trial(&self) -> Option<&TrialApplication> {
        match &self.application {
            Application::Trial(application) => Some(application),
            Application::Accepted(_) => None,
        }
    }
    /// Select storage from actual installed/provisioned identity on an admitted finite worker.
    /// Explicit other backends retain external storage settings. No borrowed path grants ownership.
    /// # Errors
    /// Preserves native/control causes and rejects redirected or changing requested directories.
    pub fn backend_storage(
        &self,
        directory: &Path,
        context: &JobContext,
    ) -> io::Result<super::BackendStorage<'_>> {
        self.backend.select(directory, context)
    }
    /// Repair launch access under the same accepted/trial owner without exposing its enum state.
    /// # Errors
    /// Preserves the authoritative publication owner's native/selection failures.
    pub fn repair_launch_entry(
        &self,
        context: &JobContext,
    ) -> Result<super::InstalledApplication, super::SelectionFailure> {
        match &self.application {
            Application::Accepted(application) => application.repair_launch_entry(context),
            Application::Trial(application) => application.repair_launch_entry(context),
        }
    }
}

/// Retain an accepted or explicitly transferred trial for the elected coordinator's lifetime.
/// The caller has already validated its running image and generation through coordinator
/// election. Call only for the elected owner, after repeated launches can forward to it.
/// Source-tree launches and native installations predating protected selection return `None`.
/// A present selection namespace never falls back to that earlier installation behavior.
/// # Errors
/// Rejects altered/missing selection, an unaccepted generation without transfer, unresolved trials, changed
/// payloads and competing run ownership. Use `LAUNCH_RESERVATION_BYTES` on an admitted worker;
/// retain the result until actual coordinator and descendant retirement is complete.
pub fn retain_running_generation(
    generation: &Path,
    inherited: Option<InheritedLaunch>,
    context: &JobContext,
) -> io::Result<Option<RunningApplication>> {
    check(context)?;
    crate::durable_storage::files::lexical(generation)?;
    let Some(releases) = generation.parent().filter(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("releases"))
    }) else {
        if inherited.is_some() {
            return Err(invalid("Inherited launch requires an installed generation"));
        }
        return Ok(None);
    };
    let root = releases.parent().ok_or_else(|| invalid("Installation root is absent"))?;
    let directory = Directory::open(root, false)?;
    match super::ownership::read(&directory) {
        Ok(_) => {}
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            if inherited.is_some() {
                return Err(invalid("Inherited launch installation ownership is missing"));
            }
            let state = super::ownership::state_path(&directory);
            for path in [
                state.join(super::selection::DIRECTORY),
                state.join(super::selection::NAMESPACE_MARKER),
            ] {
                match crate::durable_storage::files::inspect(path, context) {
                    Err(missing) if missing.kind() == io::ErrorKind::NotFound => {}
                    Err(cause) => return Err(cause),
                    Ok(_) => return Err(invalid("Installation ownership marker is missing")),
                }
            }
            return Ok(None);
        }
        Err(cause) => return Err(cause),
    }
    let Some(store) = SelectionStore::existing(root, context).map_err(io::Error::other)? else {
        if inherited.is_some() {
            return Err(invalid("Inherited launch selection is missing"));
        }
        return Ok(None);
    };
    let store = Arc::new(store);
    let inherited = match inherited {
        Some(InheritedLaunch::Trial(trial)) => {
            let application = InstallationRun::receive_trial(store, trial, generation, context)
                .map_err(io::Error::other)?;
            let generation = Directory::open(application.directory(), false)?;
            let backend = super::backend_data::BackendDataAuthority::read(
                &directory,
                &generation,
                &application.attempt().generation,
                context,
            )?;
            return Ok(Some(RunningApplication {
                application: Application::Trial(Box::new(application)),
                backend,
            }));
        }
        Some(InheritedLaunch::Selection(selection)) => Some(selection),
        None => None,
    };
    let mut run = InstallationRun::acquire(store, context).map_err(io::Error::other)?;
    let inherited_selection = inherited.is_some();
    let application = match run.select_launch(inherited, context).map_err(io::Error::other)? {
        InstallationLaunch::Application(application) => application,
        InstallationLaunch::NeedsInstallation => {
            return Err(invalid("Installation has no accepted application generation"));
        }
    };
    let actual = Directory::open(generation, false)?;
    let selected = Directory::open(application.directory(), false)?;
    if actual.identity()? != selected.identity()? {
        return Err(invalid("Running generation differs from the accepted installation"));
    }
    check(context)?;
    tracing::info!(
        operation = "installation_run_retained",
        inherited_selection,
        generation = application.generation().id.as_str(),
        "Elected coordinator retained its accepted installation generation"
    );
    let backend = super::backend_data::BackendDataAuthority::read(
        &directory,
        &selected,
        application.generation(),
        context,
    )?;
    Ok(Some(RunningApplication { application: Application::Accepted(application), backend }))
}
