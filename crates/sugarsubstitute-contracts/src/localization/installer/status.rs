// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Closed worker product-message contracts; raw causes belong to diagnostics.
use super::InstallerMessages;
use serde::Deserialize;

/// Owned progress identities accepted from the authenticated provisioning worker.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProgressMessage {
    /// Validate the selected Python runtime.
    CheckingPython,
    /// Inspect the selected SugarCubes dependency set.
    CheckingSugarCubes,
    /// Install missing dependencies.
    InstallingSugarCubes,
    /// Confirm the dependency result.
    SugarCubesReady,
}
impl ProgressMessage {
    /// Resolve only the owned progress surface; failure codes cannot be used here.
    pub fn text(self, copy: &InstallerMessages) -> &'static str {
        match self {
            Self::CheckingPython => copy.checking_python,
            Self::CheckingSugarCubes => copy.checking_sugar_cubes,
            Self::InstallingSugarCubes => copy.installing_sugar_cubes,
            Self::SugarCubesReady => copy.sugar_cubes_ready,
        }
    }
}

/// Actionable installer failure categories; unknown codes fail closed at the host boundary.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FailureMessage {
    /// General failure with original diagnostic evidence retained separately.
    SetupFailed,
    /// Shared storage could not admit more data.
    CacheFull,
    /// The attached installation needs an explicit Python selection.
    SelectPython,
}
impl FailureMessage {
    /// Resolve product feedback without formatting an internal cause into product copy.
    pub fn text(self, copy: &InstallerMessages) -> &'static str {
        match self {
            Self::SetupFailed => copy.setup_failed,
            Self::CacheFull => copy.cache_full,
            Self::SelectPython => copy.select_python,
        }
    }
}
