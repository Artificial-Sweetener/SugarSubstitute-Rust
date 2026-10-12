// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare bounded launch inputs and own contained Windows process families.
//!
//! Prepared input buffers establish lexical shape and argument fidelity. Native
//! callers separately establish canonical trust and authenticated launch authority.
//! The Windows registry retains native families through cancellation and observed cleanup.

#[cfg(windows)]
pub mod backend_listener;
#[cfg(windows)]
pub mod installed_frontend;

mod arguments;
mod command;
mod errors;

#[cfg(windows)]
mod claim_error;
#[cfg(windows)]
mod claim_types;
#[cfg(windows)]
mod claims;
#[cfg(windows)]
pub mod diagnostic_storage;
#[cfg(windows)]
mod environment;
#[cfg(windows)]
pub mod fixture_descendant;
#[cfg(windows)]
pub mod lifecycle_protocol;
#[cfg(windows)]
mod managed_output;
#[cfg(windows)]
#[allow(unsafe_code)]
mod native_claims;
#[cfg(windows)]
pub mod profiles;
#[cfg(windows)]
#[allow(unsafe_code)]
mod protected_storage;
#[cfg(windows)]
pub use protected_storage::{ProtectedStorageGeneration, ProtectedStorageRoot};
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
pub use claim_error::{ClaimAdmissionError, ClaimError};
#[cfg(windows)]
pub use claim_types::{
    CLAIM_POOL_BYTES, ClaimLaunchJobs, ClaimObservation, ClaimTarget, ManagedLaunchInput,
    ResolutionTicket,
};
#[cfg(windows)]
pub use claims::{CLAIM_OPERATION_BYTES, WindowsResourceClaims};
#[cfg(windows)]
pub use environment::{EnvironmentOverride, LaunchEnvironment, LaunchEnvironmentError};
#[cfg(windows)]
pub use managed_output::RejectedManagedOutputLaunch;
#[cfg(windows)]
pub use registry::WindowsProcessRegistry;
#[cfg(windows)]
pub use registry_error::{RegistryAdmissionError, RegistryError, RegistryInputError};
#[cfg(windows)]
pub use registry_input::{RejectedOutputLaunch, WindowsLaunchInput};
#[cfg(windows)]
pub use registry_types::{
    FamilyBinding, LaunchTicket, RegistryObservation, RegistrySnapshot, RootObservationOutcome,
};
