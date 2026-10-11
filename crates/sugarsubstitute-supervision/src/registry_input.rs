// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move bounded native inputs once and account actual retained capacities before admission.
use crate::{LaunchEnvironment, RegistryInputError, WindowsLaunchBuffers, WindowsLaunchCommand};
use std::fmt;

// Fixed family state, 256 retained members, aligned attribute storage and query scratch.
// This is an admission peak, not an estimate of unobservable kernel allocations.
pub(crate) const FAMILY_STORAGE_BYTES: u64 = 131_072;
/// Own explicit executable/cwd/argv/environment for one admitted creation operation.
/// Values establish input shape only; a trusted native caller supplies ownership.
pub struct WindowsLaunchInput {
    pub(crate) command: WindowsLaunchBuffers,
    pub(crate) environment: LaunchEnvironment,
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
        Ok(Self { command, environment, required })
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
