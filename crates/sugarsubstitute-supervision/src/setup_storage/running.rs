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
pub enum RunningApplication {
    /// Previously health-accepted generation.
    Accepted(Box<AcceptedApplication>),
    /// Live transferred attempt awaiting authenticated application readiness.
    Trial(Box<TrialApplication>),
}
impl RunningApplication {
    /// Exact verified generation retained by this coordinator.
    pub fn generation(&self) -> &sugarsubstitute_contracts::installation::Generation {
        match self {
            Self::Accepted(application) => application.generation(),
            Self::Trial(application) => &application.attempt().generation,
        }
    }
    /// Verified running directory whose payload remains protected by this owner.
    pub fn directory(&self) -> &Path {
        match self {
            Self::Accepted(application) => application.directory(),
            Self::Trial(application) => application.directory(),
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
    match directory.read(super::ownership::MARKER) {
        Ok(_) => {}
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            if inherited.is_some() {
                return Err(invalid("Inherited launch installation ownership is missing"));
            }
            for leaf in [".selection-state", super::selection::NAMESPACE_MARKER] {
                match std::fs::symlink_metadata(root.join(leaf)) {
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
            return Ok(Some(RunningApplication::Trial(Box::new(application))));
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
    Ok(Some(RunningApplication::Accepted(application)))
}
