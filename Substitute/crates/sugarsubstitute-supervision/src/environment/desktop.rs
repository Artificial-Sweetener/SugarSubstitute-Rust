// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the desktop child's environment: the user's own.
use super::{EnvironmentOverride, LaunchEnvironment, LaunchEnvironmentError};

impl LaunchEnvironment {
    /// The desktop child runs in the user's own profile environment, so Windows dialogs and
    /// known folders resolve to the user's real Desktop, Documents and settings.
    ///
    /// Home, application-data and temporary-folder variables keep their captured values. The
    /// desktop child names every folder it writes explicitly: its browser profile, input-canvas
    /// archives and diagnostics live under the installation's `user\` folder. Inherited WebView2
    /// data-folder and browser-argument overrides are removed, so they cannot move the browser
    /// profile; a qualification build keeps a lone remote-debugging port so a proof can drive
    /// the page. Processes that need confined storage, such as an installation-owned ComfyUI,
    /// receive it from [`Self::for_local_storage`].
    /// # Errors
    /// Preserves malformed snapshot, ordinal, allocation and bound causes.
    pub fn for_desktop(snapshot: &[u16]) -> Result<Self, LaunchEnvironmentError> {
        let names = super::storage::removed_browser_overrides(snapshot)
            .iter()
            .map(|name| name.encode_utf16().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let removals =
            names.iter().map(|name| EnvironmentOverride::Remove { name }).collect::<Vec<_>>();
        Self::from_snapshot_utf16(snapshot, &removals)
    }
}
