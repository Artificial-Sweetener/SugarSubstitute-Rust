// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move one exact admitted desktop channel into its contained launch.
use super::*;
use sugarsubstitute_contracts::lifecycle::RunKey;
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecycleDesktopEnrollment, LifecyclePipeBootstrap,
};

pub(crate) struct DesktopLaunch {
    pub(crate) bootstrap: LifecyclePipeBootstrap,
    pub(crate) enrollment: Option<LifecycleDesktopEnrollment>,
    pub(crate) run: RunKey,
}
impl WindowsLaunchInput {
    /// Bind a single read-only section and its exact desktop enrollment session.
    /// The registry accepts this input only for the original `Desktop(run)` binding.
    /// Pins remain with the native family through attribute deletion; execution
    /// retains the channel and its actual retirement. No native effect occurs here.
    ///
    /// # Errors
    /// Rejects mismatched runs or channel provenance before buffer accounting.
    /// Rejected pins request channel cancellation without performing native close.
    /// Buffer accounting returns the same checked errors as [`Self::new`].
    pub fn with_lifecycle_pipe(
        command: WindowsLaunchCommand,
        environment: LaunchEnvironment,
        bootstrap: LifecyclePipeBootstrap,
        enrollment: LifecycleDesktopEnrollment,
    ) -> Result<Self, RegistryInputError> {
        if !bootstrap.matches_enrollment(&enrollment) {
            return Err(RegistryInputError::LifecycleProvenance);
        }
        let run = enrollment.run();
        let mut input = Self::new(command, environment)?;
        input.lifecycle = Some(DesktopLaunch { bootstrap, enrollment: Some(enrollment), run });
        Ok(input)
    }
}
