// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select explicit inputs or defer installed descriptor reading until admitted preparation.
use std::path::PathBuf;
use sugarsubstitute_configuration::paths::StartupPaths;
/// Inert source selection retains all original inputs until preparation admission.
pub(crate) enum ConfigurationSource {
    Explicit(StartupPaths),
    Installed(PathBuf),
}
impl From<StartupPaths> for ConfigurationSource {
    fn from(paths: StartupPaths) -> Self {
        Self::Explicit(paths)
    }
}
