// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one original preparation's native block and copied data through checked transfer.
//!
//! The copied-unit limit is request policy. The operating system block has an
//! unmeasured extent; its lifetime never proves a physical memory peak bound.

mod access;
mod capture;
pub(crate) mod cause;
mod context;
pub(crate) mod control;
pub(crate) mod core_binding;
mod custody;
mod environment;
pub(crate) mod error;
mod input_error;
mod layout;
mod memory;
mod outcome;
pub(crate) mod owner;
mod pending;
pub(crate) mod port;
mod request;
#[cfg(any(windows, test))]
pub(crate) mod retirement;
#[cfg(any(windows, test))]
mod runner;
pub(crate) mod slot;
mod source_cache;
mod state;
#[cfg(any(windows, test))]
pub(crate) mod submission;
mod success;

pub use context::EntryPreparationContext;
pub use control::EntryPreparationControlCause;
pub use environment::{
    CapturedEntryEnvironment, EntryEnvironmentFailure, EntryEnvironmentObservation,
};
pub use error::EntryPreparationError;
pub use input_error::EntryPreparationInputError;
pub use outcome::{EntryPreparationCause, EntryPreparationFailure};
pub(crate) use owner::EntryPreparationOwner;
pub use request::{EntryRequestAdoptionRejection, EntryRequestInput};
pub use success::EntryPreparationSuccess;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod core_tests;
#[cfg(test)]
mod queue_tests;

#[cfg(test)]
pub(crate) mod staging_support;

#[cfg(test)]
mod client_liveness_tests;

#[cfg(test)]
mod cache_tests;
