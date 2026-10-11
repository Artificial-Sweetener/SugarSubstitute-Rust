// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare bounded Windows launch inputs independently of process authority.
//!
//! Prepared input buffers establish lexical shape and argument fidelity. Native
//! adapters separately establish canonical trust, containment and actual lifetime.

mod arguments;
mod command;
mod errors;

#[cfg(windows)]
mod environment;

pub use command::{MAX_WINDOWS_ARGUMENTS, WindowsLaunchBuffers, WindowsLaunchCommand};
pub use errors::LaunchCommandError;

#[cfg(windows)]
pub use environment::{EnvironmentOverride, LaunchEnvironment, LaunchEnvironmentError};
