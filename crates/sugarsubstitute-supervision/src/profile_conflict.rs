// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve a user-selected profile conflict through retained native process identities.
mod native;
mod processes;
mod resources;
use crate::ClaimError;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::execution::JobContext;

/// Inert profile identity captured by the original protected acquisition.
#[derive(Clone, Debug)]
pub struct ConflictRequest {
    pub(crate) path: PathBuf,
    pub(crate) folder: [u8; 24],
    pub(crate) account: [u8; 32],
}

use sugarsubstitute_contracts::instance_conflict::{
    ConflictBackend, ConflictObservation, ConflictOperation,
};
/// Preserve native outputs across worker cancellation, errors and result disposal.
#[derive(Clone)]
pub struct ConflictSession {
    native: Arc<Mutex<native::Native>>,
    relaunch: Arc<Mutex<Option<crate::application_relaunch::ApplicationRelaunch>>>,
}
impl ConflictSession {
    /// Construct inert state. Native operations must run in an admitted blocking job.
    pub fn new(request: ConflictRequest) -> Self {
        Self {
            native: Arc::new(Mutex::new(native::Native::new(request))),
            relaunch: Arc::new(Mutex::new(None)),
        }
    }
}
impl ConflictBackend for ConflictSession {
    type Error = ClaimError;
    /// Execute one bounded operation, retaining all partial outputs for explicit retirement.
    /// # Errors
    /// Rejects changed identity, unknown owners, expired jobs and native failures.
    fn execute(
        &self,
        operation: ConflictOperation,
        context: &JobContext,
    ) -> Result<ConflictObservation, ClaimError> {
        let mut state = crate::native_claims::lock(&self.native);
        state.execute(operation, context)
    }
    fn relaunch(&self, context: &JobContext) -> Result<(), ClaimError> {
        use std::os::windows::ffi::OsStrExt;
        let mut retained = crate::native_claims::lock(&self.relaunch);
        if retained.is_some() {
            return Err(ClaimError::invalid("application_relaunch_duplicate"));
        }
        let executable = std::env::current_exe()
            .map_err(|cause| ClaimError::new("application_relaunch_executable", cause))?;
        let directory = std::env::current_dir()
            .map_err(|cause| ClaimError::new("application_relaunch_directory", cause))?;
        let command = crate::WindowsLaunchCommand::from_utf16(
            &executable.as_os_str().encode_wide().collect::<Vec<_>>(),
            &directory.as_os_str().encode_wide().collect::<Vec<_>>(),
            &std::env::args_os().skip(1).map(|arg| arg.encode_wide().collect()).collect::<Vec<_>>(),
        )
        .map_err(|cause| ClaimError::new("application_relaunch_command", cause))?;
        let launch = crate::application_relaunch::ApplicationRelaunch::new(command);
        *retained = Some(launch.clone());
        launch.launch(context)
    }
    fn retire_relaunch(&self) -> Result<(), ClaimError> {
        if let Some(launch) = &*crate::native_claims::lock(&self.relaunch) {
            launch.retire()?;
        }
        Ok(())
    }
}

/// A finite operation checks cancellation before every externally visible action.
fn check(context: &JobContext) -> Result<(), ClaimError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        Err(ClaimError::invalid("profile_conflict_operation_expired"))
    } else {
        Ok(())
    }
}
