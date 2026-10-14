// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate actual finite entry discovery/election through the execution owner.
//! Cached UI methods never perform native work or wait for workers. Native custody
//! survives cancellation, late completion, failed closure and abandoned owner loss.
pub(crate) mod election;
#[cfg(test)]
pub(crate) mod fixture;
mod input;
pub(crate) mod namespace;
#[cfg(feature = "boundary-fixtures")]
#[path = "coordinator_entry/namespace_fixture.rs"]
pub mod namespace_fixture;
mod native;
mod owner;
pub(crate) mod read_namespace;
#[cfg(test)]
mod tests;
mod types;
pub use input::CoordinatorEntryInput;
pub(crate) use namespace::NamespaceLifetime;
pub use namespace::VerifiedEntryNamespace;
pub use owner::CoordinatorEntryOwner;
pub use types::{
    EntryAdmissionError, EntryError, EntryInputError, EntryObservation, EntryPhase, EntrySnapshot,
};
/// Exact retained reservation for finite namespace/image/election custody, in bytes.
pub const ENTRY_PREPARATION_BYTES: u64 = 2 * 1024 * 1024;
