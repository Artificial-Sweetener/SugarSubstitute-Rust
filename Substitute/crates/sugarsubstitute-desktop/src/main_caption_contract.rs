// SPDX-License-Identifier: GPL-3.0-or-later
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

/// Borrow canonical localized connection-retry copy without allocating product text.
#[derive(Clone, Copy, Debug)]
pub struct MainRecoveryLabels {
    /// Existing localized retry action.
    pub retry: &'static str,
    /// Existing localized actionable retry-ready status.
    pub retry_ready: &'static str,
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

/// Whether the managed backend is running a workflow, as the application observes it.
/// The restart offer reads "after this finishes" while one runs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorkflowActivity {
    /// No workflow is queued or running, or the host cannot tell.
    #[default]
    Idle,
    /// A workflow is queued or running.
    Running,
}

/// A one-time message about updates for the first start after one, shown as one caption line
/// the user can dismiss.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateNotice {
    /// This is the first start on a new version: "Updated to version X", with a link to its
    /// release notes when the host knows where they are. Only `https` links are opened.
    Updated {
        /// The version now running.
        version: String,
        /// The release notes of that version.
        changelog: Option<url::Url>,
    },
    /// The new version did not start, so the one before it is running.
    PreviousVersionRunning,
}
