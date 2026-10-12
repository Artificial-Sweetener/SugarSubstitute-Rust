// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound independent desktop invocation facts without manufacturing coordinator authority.
//!
//! These inert values describe requests and protocol observations. Actual native
//! peer verification and retained election/claim admission belong to adapters.

mod deadline;
mod errors;
mod identity;
mod input;

pub use deadline::EntrySetupBudget;
pub use errors::{EntryBudgetError, EntryOwnerDegradation, EntryRejection, EntryShapeError};
pub use identity::{
    EntryConnectionId, EntryDiscoveryPhase, EntryNativeIdentityFacts, EntryOperationId,
    EntryOutcome, EntryRequestId,
};
pub use input::{
    EntryArgumentsIter, EntryArgumentsView, EntryConnectionView, EntryEnvironmentView,
    EntryFrontendView, EntryLaunchParts, EntryLaunchRequest, EntryLaunchRequestView,
    EntryWindowsPathView, MAX_ENTRY_ENVIRONMENT_UNITS, MAX_ENTRY_PATH_UNITS,
    MAX_ENTRY_REQUEST_ALLOCATION_BYTES,
};

#[cfg(test)]
mod tests;
