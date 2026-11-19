// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What the main window's caption shows besides its window buttons: localized names, backend
//! recovery and whether a workflow is running, as the application reports them.

/// Borrow localized native caption names from the authoritative locale owner.
#[derive(Clone, Copy, Debug)]
pub struct MainCaptionLabels {
    /// Accessible minimize control name.
    pub minimize: &'static str,
    /// Accessible maximize control name.
    pub maximize: &'static str,
    /// Accessible restore control name.
    pub restore: &'static str,
    /// Accessible close control name.
    pub close: &'static str,
}

/// Present current backend recovery data; this state grants no backend admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainBackendState {
    /// Genuine old backend ownership remains unsettled; retry stays hidden.
    Retiring,
    /// The composing owner reports actual old settlement and retained cause.
    RetryReady,
    /// An explicit retry or shared-cohort successor is pending; retry is disabled.
    Retrying,
    /// The composing owner accepted a current same-origin replacement for this failed lease.
    Recovered,
}

/// A one-time message about updates for the first start after one, which the page shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateNotice {
    /// This is the first start on a new version: "Updated to version X".
    Updated {
        /// The version now running.
        version: String,
    },
    /// The new version did not start, so the one before it is running.
    PreviousVersionRunning,
    /// The newer version's files were damaged, so the one before it is running while the
    /// update is downloaded again.
    Repairing,
    /// Another program changed the update storage, so it was set aside and updates were set
    /// up again.
    StorageSetAside,
    /// The start's SugarCubes update has one line to say, such as that it couldn't check.
    Sugarcubes(sugarsubstitute_contracts::update::pack::PackUpdateReport),
}
