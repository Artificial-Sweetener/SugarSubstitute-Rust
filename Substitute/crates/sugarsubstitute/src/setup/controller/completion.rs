// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Release completed setup custody and independently reacquire launch authority for Open.
use super::*;
use setup_storage::{InstalledCandidate, ReservedLaunch};

/// Reopening returns creator custody before the UI can admit transfer or process creation.
pub(super) enum OpenedLaunch {
    Installed(ApplicationLaunch),
    Accepted(ApplicationLaunch),
}
impl OpenedLaunch {
    /// Retain either creator through cancellation and observed retirement.
    pub(super) fn into_launch(self) -> ApplicationLaunch {
        match self {
            Self::Installed(launch) | Self::Accepted(launch) => launch,
        }
    }
}

impl Controller {
    /// Ready is possible only after durable authorization and both shell entries are published.
    pub(super) fn publish_installation(&mut self, candidate: SignedCandidate) -> Result<()> {
        self.submit(StorageWork::Publish, move |context| {
            let installed = crate::authority_clock::authorize(|now| {
                candidate.authorize_installation(now, &context)
            })?;
            installed.publish(&context).map(StorageResult::Installed)
        })
    }

    /// Revalidate the installation after Ready; a shortcut may have launched it in the meantime.
    pub(super) fn open_installation(&mut self, completed: CompletedInstallation) -> Result<()> {
        self.submit(StorageWork::Open, move |context| {
            reopen(completed.root(), &context).map(StorageResult::Opened)
        })
    }

    /// Retain creator custody before admission, including if the next job cannot be submitted.
    pub(super) fn opened_installation(&mut self, opened: OpenedLaunch) -> Result<()> {
        let trial = matches!(&opened, OpenedLaunch::Installed(_));
        let launch = opened.into_launch();
        self.relaunch = Some(launch.clone());
        if trial {
            self.submit(StorageWork::Transfer, move |context| {
                crate::authority_clock::authorize(|now| launch.prepare_trial(now, &context))?;
                Ok(StorageResult::Transferred)
            })
        } else {
            self.submit(StorageWork::Launch, move |context| {
                launch.launch(&context).map_err(io::Error::other)?;
                Ok(StorageResult::Launched)
            })
        }
    }
}

/// Resolve current protected state without using the displayed generation as executable authority.
fn reopen(root: &std::path::Path, context: &JobContext) -> io::Result<OpenedLaunch> {
    if let Some(installed) = InstalledCandidate::reopen(root, context)? {
        return Ok(OpenedLaunch::Installed(ApplicationLaunch::installed(Arc::new(installed))));
    }
    match setup_storage::reserve_installed_launch(root, context).map_err(io::Error::other)? {
        ReservedLaunch::Application(application) => {
            ApplicationLaunch::selected(application.prepare_inheritance(context)?)
                .map(OpenedLaunch::Accepted)
                .map_err(io::Error::other)
        }
        ReservedLaunch::NeedsInstallation => Err(io::Error::other(
            "Installation has no launchable application; run setup to repair it",
        )),
        ReservedLaunch::ReconcileTrial(_) => {
            Err(io::Error::other("Installation trial requires native process reconciliation"))
        }
    }
}
