// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind actual helper release receipts to the registry transfer capsule.
use super::*;
impl WindowsProcessRegistry {
    /// Retain the actual same-controller release receipt for checked close-only cleanup.
    /// # Errors
    /// Rejects foreign parent/resource bindings without synthesizing a receipt.
    pub fn observe_backend_custody_release(
        &mut self,
        parent: &crate::BackendCustodyParent,
        ticket: LaunchTicket,
    ) -> Result<bool, RegistryAdmissionError> {
        let FamilyBinding::Managed(resource) = ticket.binding else {
            return Err(RegistryAdmissionError::InvalidSpecification);
        };
        if parent.key().owner() != resource.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        let at = self.index(ticket.process)?;
        let record = self.records[at].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if record.ticket != ticket {
            return Err(RegistryAdmissionError::Stale);
        }
        if record.creation.is_some()
            || record.establishment_job.is_some()
            || record.cleanup.is_some()
        {
            return Ok(false);
        }
        let mut state = lock(&record.shared.native);
        if let Some(ack) = parent.release_acknowledgement(resource) {
            state.custody_release = Some(ack);
        }
        Ok(state.custody_destination.is_none())
    }
    /// Borrow whether original transfer metadata still needs checked retirement.
    /// This is a cached hint and grants no native effect authority.
    /// # Errors
    /// Rejects stale or foreign process identities.
    pub fn custody_retirement_pending(
        &self,
        key: ProcessKey,
    ) -> Result<bool, RegistryAdmissionError> {
        let at = self.index(key)?;
        Ok(self.records[at]
            .as_ref()
            .ok_or(RegistryAdmissionError::Stale)?
            .custody_retirement_pending())
    }
}
