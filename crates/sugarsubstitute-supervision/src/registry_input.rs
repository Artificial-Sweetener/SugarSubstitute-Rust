// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move bounded native inputs once and account actual retained capacities before admission.
use crate::{LaunchEnvironment, RegistryInputError, WindowsLaunchBuffers, WindowsLaunchCommand};
use std::fmt;
use sugarsubstitute_execution::windows_fixture::FixtureChildEvents;
pub(crate) mod backend_custody;
mod diagnostic;
mod lifecycle;
pub(crate) mod output;
use diagnostic::DiagnosticLaunch;
use lifecycle::DesktopLaunch;
pub use output::RejectedOutputLaunch;

// Fixed family state, 256 retained members, aligned attribute storage and query scratch.
// This is an admission peak, not an estimate of unobservable kernel allocations.
pub(crate) const FAMILY_STORAGE_BYTES: u64 = 131_072;
/// Own explicit executable/cwd/argv/environment for one admitted creation operation.
/// Values establish input shape only; a trusted native caller supplies ownership.
pub struct WindowsLaunchInput {
    pub(crate) profile_custody: Option<crate::registry::profile::prepared::PreparedDesktopCarrier>,
    pub(crate) managed_custody: Option<crate::claims::managed_custody::ManagedCustodySource>,
    pub(crate) command: WindowsLaunchBuffers,
    pub(crate) environment: LaunchEnvironment,
    pub(crate) fixture_events: Option<FixtureChildEvents>,
    pub(crate) lifecycle: Option<DesktopLaunch>,
    pub(crate) diagnostic: Option<DiagnosticLaunch>,
    pub(crate) backend_custody: Option<backend_custody::CustodyLaunch>,
    pub(crate) output: Option<output::OutputLaunch>,
    required: u64,
}
impl WindowsLaunchInput {
    /// Move validated input and compute checked actual allocation-capacity accounting.
    /// No native API, environment capture, execution admission or process effect occurs.
    pub fn new(
        command: WindowsLaunchCommand,
        environment: LaunchEnvironment,
    ) -> Result<Self, RegistryInputError> {
        let command = command.into_native_parts();
        let mut required = FAMILY_STORAGE_BYTES;
        for bytes in [
            command.application_name.capacity(),
            command.working_directory.capacity(),
            command.command_line.capacity(),
        ] {
            required = required
                .checked_add(
                    u64::try_from(bytes)
                        .map_err(|_| RegistryInputError::Capacity)?
                        .checked_mul(2)
                        .ok_or(RegistryInputError::Capacity)?,
                )
                .ok_or(RegistryInputError::Capacity)?;
        }
        required = required
            .checked_add(environment.allocated_bytes().map_err(RegistryInputError::Environment)?)
            .ok_or(RegistryInputError::Capacity)?;
        Ok(Self {
            profile_custody: None,
            managed_custody: None,
            command,
            environment,
            fixture_events: None,
            lifecycle: None,
            diagnostic: None,
            backend_custody: None,
            output: None,
            required,
        })
    }
    /// Move an execution-owned four-event launch pin into this explicit fixture input.
    ///
    /// The contained launch adapter retains the pin through native attribute-list
    /// deletion, including partial creation or resume failures. Event storage remains
    /// charged to its original session; this input accounts its own buffers and family.
    /// Ordinary input has an empty inheritance allowlist. The trusted caller supplies
    /// the dedicated child role and its exact handle tokens as command arguments.
    ///
    /// # Errors
    /// Returns the same buffer-capacity/allocation errors as [`Self::new`]. A rejected
    /// input releases its pin without a native close; the execution owner retires it.
    pub fn with_fixture_events(
        command: WindowsLaunchCommand,
        environment: LaunchEnvironment,
        events: FixtureChildEvents,
    ) -> Result<Self, RegistryInputError> {
        let mut input = Self::new(command, environment)?;
        input.fixture_events = Some(events);
        Ok(input)
    }
    /// Return the required real creation-job peak reservation, in bytes.
    /// The worker retains/claims that admitted JobContext lease before native effects.
    pub const fn required_reservation_bytes(&self) -> u64 {
        self.required
    }
}
impl fmt::Debug for WindowsLaunchInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowsLaunchInput")
            .field("required_bytes", &self.required)
            .finish_non_exhaustive()
    }
}
