// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Build the existing Desktop role inside the admitted worker after its real section exists.
use crate::production::{
    entry_activation::desktop_role::{self, DesktopRoleSource},
    qualification::QualificationIntent,
};
use std::{error::Error, ffi::OsStr, os::windows::ffi::OsStrExt};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_execution::{
    recovery_supervisor::AuthenticatedRecoveryOrigin,
    windows_lifecycle_pipe::RecoveryDesktopBootstrap,
};
use sugarsubstitute_supervision::{LaunchEnvironment, WindowsLaunchCommand, WindowsLaunchInput};
/// Preserve the original configuration, path, grammar or allocation cause from the worker.
#[derive(Debug)]
pub(super) struct Failure(Box<dyn Error + Send + Sync>);
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Actual recovery Desktop command preparation failed")
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}
/// No path/environment discovery or owned argument copying occurs on the headless main thread.
/// The native driver has already claimed its original bounded transient reservation.
pub(super) fn build(
    origin: &AuthenticatedRecoveryOrigin,
    bootstrap: &RecoveryDesktopBootstrap,
    context: &JobContext,
) -> Result<WindowsLaunchInput, Failure> {
    prepare(origin, bootstrap, context).map_err(Failure)
}
fn prepare(
    origin: &AuthenticatedRecoveryOrigin,
    bootstrap: &RecoveryDesktopBootstrap,
    context: &JobContext,
) -> Result<WindowsLaunchInput, Box<dyn Error + Send + Sync>> {
    origin.check()?;
    if context.cancellation_requested()
        || context
            .deadline()
            .is_none_or(|until| context.now() >= until || until > origin.original_until())
    {
        return Err("Original recovery Desktop command admission stopped".into());
    }
    let identity = origin.desktop_identity()?;
    let arguments = desktop_role::arguments(
        DesktopRoleSource::Recovery(bootstrap),
        identity,
        &QualificationIntent::default(),
    )?;
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or("Actual recovery executable directory missing")?;
    let command = WindowsLaunchCommand::from_utf16(
        &wide(executable.as_os_str())?,
        &wide(directory.as_os_str())?,
        &arguments,
    )?;
    let requested = origin.view()?.environment();
    let mut snapshot = Vec::new();
    snapshot.try_reserve_exact(requested.unit_len())?;
    snapshot.extend(requested.units());
    let environment = LaunchEnvironment::from_snapshot_utf16(&snapshot, &[])?;
    origin.check()?;
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        return Err("Original recovery Desktop command preparation expired".into());
    }
    Ok(WindowsLaunchInput::new(command, environment)?)
}
/// Account exact bounded units before retaining a Windows path copy.
fn wide(value: &OsStr) -> Result<Vec<u16>, std::collections::TryReserveError> {
    let mut output = Vec::new();
    output.try_reserve_exact(value.encode_wide().count())?;
    output.extend(value.encode_wide());
    Ok(output)
}
