// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare bounded launch inputs and own contained Windows process families.
//!
//! Prepared input buffers establish lexical shape and argument fidelity. Native
//! callers separately establish canonical trust and authenticated launch authority.
//! The Windows registry retains native families through cancellation and observed cleanup.

mod arguments;
mod command;
mod errors;

#[cfg(windows)]
mod environment;
#[cfg(windows)]
#[allow(unsafe_code)]
mod native_process;
#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod registry_error;
#[cfg(windows)]
mod registry_input;
#[cfg(windows)]
mod registry_types;

pub use command::{MAX_WINDOWS_ARGUMENTS, WindowsLaunchBuffers, WindowsLaunchCommand};
pub use errors::LaunchCommandError;

#[cfg(windows)]
pub use environment::{EnvironmentOverride, LaunchEnvironment, LaunchEnvironmentError};
#[cfg(windows)]
pub use registry::WindowsProcessRegistry;
#[cfg(windows)]
pub use registry_error::{RegistryAdmissionError, RegistryError, RegistryInputError};
#[cfg(windows)]
pub use registry_input::WindowsLaunchInput;
#[cfg(windows)]
pub use registry_types::{FamilyBinding, LaunchTicket, RegistryObservation, RegistrySnapshot};
