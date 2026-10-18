// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hand back a charged original source only after an unsubmitted session retires.
use super::*;
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, lifecycle::CoordinatorEpoch};

/// Sealed source handback; absence grants no coordinator or process authority.
pub struct UnsubmittedEntryRequest {
    pub(super) input: Box<CoordinatorEntryClientInput>,
}
impl UnsubmittedEntryRequest {
    /// Validate exact namespace/candidate and original preparation without consuming custody.
    /// # Errors
    /// Preserves the actual original preparation failure; mismatched identities are rejected.
    pub fn validate(
        &self,
        facts: &sugarsubstitute_contracts::coordinator_entry::EntryNativeIdentityFacts,
        candidate: Option<CoordinatorEpoch>,
        source: sugarsubstitute_contracts::execution::CounterDeadline,
        client: &crate::ExecutionClient,
        original: EntrySetupBudget,
    ) -> Result<(), EntryTransportError> {
        if self.input.facts != *facts
            || self.input.candidate != candidate
            || self.input.source_setup != source
        {
            return Err(EntryTransportError::Authentication);
        }
        self.input
            .request
            .validate_original(client, original)
            .map_err(EntryTransportError::Preparation)
    }

    /// Transfer unchanged request data and retained storage after checked validation.
    pub fn into_request(self) -> crate::EntryRequestInput {
        self.input.request
    }
}
impl CoordinatorEntryControl {
    /// Take a source that never dispatched Launch and completed native session retirement.
    /// Terminal publication follows handback publication; callers must adopt the matching
    /// successful terminal before attempting a new election under the original setup.
    pub fn take_unsubmitted_request(&self) -> Option<UnsubmittedEntryRequest> {
        lock(&self.shared.state).unsubmitted.take()
    }
}
