// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer an explicit replacement launch to a fresh, independently owned application Root.
use crate::{
    ClaimError, WindowsLaunchBuffers, WindowsLaunchCommand,
    native_claims::files::{self, Budget, Handle},
};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use windows::{
    Win32::System::Threading::*,
    core::{PCWSTR, PWSTR},
};

/// Keep creation outputs owned even if the admitted job's result is cancelled or discarded.
/// The new Root owns its own lifetime from successful creation; this owner never terminates it.
#[derive(Clone)]
pub struct ApplicationRelaunch(Arc<Mutex<State>>);
struct State {
    command: Option<WindowsLaunchBuffers>,
    issued: bool,
    handles: Vec<Handle>,
    memory: Option<Arc<MemoryClaim>>,
}
impl ApplicationRelaunch {
    /// Retain inert, already bounded executable/cwd/argv buffers. Performs no native work.
    pub fn new(command: WindowsLaunchCommand) -> Self {
        Self(Arc::new(Mutex::new(State {
            command: Some(command.into_native_parts()),
            issued: false,
            handles: Vec::new(),
            memory: None,
        })))
    }
    /// Create the replacement exactly once in an admitted blocking job, then close creator handles.
    /// The caller must have retired its original application and conflict custody first.
    /// # Errors
    /// Returns original creation/cleanup failures. `retire` closes retained partial outputs.
    pub fn launch(&self, context: &JobContext) -> Result<(), ClaimError> {
        let mut state = crate::native_claims::lock(&self.0);
        if state.issued
            || context.cancellation_requested()
            || context.deadline().is_none_or(|until| context.now() >= until)
        {
            return Err(ClaimError::invalid("application_relaunch_admission"));
        }
        state.memory = Some(Arc::new(
            context
                .memory()
                .claim(262144)
                .map_err(|cause| ClaimError::new("application_relaunch_memory", cause))?,
        ));
        state
            .handles
            .try_reserve_exact(2)
            .map_err(|cause| ClaimError::new("application_relaunch_handles", cause))?;
        let budget = Budget::Protected(
            state
                .memory
                .as_ref()
                .ok_or_else(|| ClaimError::invalid("application_relaunch_memory"))?
                .clone(),
        );
        let command = state
            .command
            .as_mut()
            .ok_or_else(|| ClaimError::invalid("application_relaunch_command"))?;
        let startup = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, ..Default::default() };
        let mut process = PROCESS_INFORMATION::default();
        // SAFETY: Bounded owned buffers live through creation. No handles are inherited, and
        // the default desktop is the caller's desktop. This is an explicit new user launch.
        unsafe {
            CreateProcessW(
                PCWSTR(command.application_name.as_ptr()),
                Some(PWSTR(command.command_line.as_mut_ptr())),
                None,
                None,
                false,
                CREATE_UNICODE_ENVIRONMENT,
                None,
                PCWSTR(command.working_directory.as_ptr()),
                &startup,
                &mut process,
            )
        }
        .map_err(|cause| ClaimError::new("application_relaunch_create", cause))?;
        state.issued = true;
        state.handles.push(Handle::new(process.hProcess, budget.clone()));
        state.handles.push(Handle::new(process.hThread, budget));
        tracing::info!(
            operation = "application_relaunch_created",
            pid = process.dwProcessId,
            "Transferred startup to a fresh application Root"
        );
        state.retire()
    }
    /// Close only this creator's retained handles; never stop the independent replacement.
    /// # Errors
    /// Preserves native close failures and their still-owned handles for another cleanup attempt.
    pub fn retire(&self) -> Result<(), ClaimError> {
        crate::native_claims::lock(&self.0).retire()
    }
}
impl State {
    fn retire(&mut self) -> Result<(), ClaimError> {
        files::close_all(&mut self.handles)?;
        self.memory = None;
        self.command = None;
        Ok(())
    }
}
