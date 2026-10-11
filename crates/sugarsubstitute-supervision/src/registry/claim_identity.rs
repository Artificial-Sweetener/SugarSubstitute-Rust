// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind claim retirement to the actual registry owner and exact managed generation.

use super::*;
use sugarsubstitute_contracts::lifecycle::ResourceKey;

/// Retain one opaque registry lifetime; matching coordinator facts cannot recreate it.
#[derive(Clone)]
pub(crate) struct RegistryIdentity(Arc<()>);

impl RegistryIdentity {
    pub(super) fn new() -> Self {
        Self(Arc::new(()))
    }

    /// Compare actual retained owners without exposing their allocation or native state.
    pub(crate) fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl WindowsProcessRegistry {
    /// Pin this registry's ownership independently of public coordinator matching facts.
    pub(crate) fn claim_identity(
        &self,
        owner: CoordinatorKey,
    ) -> Result<RegistryIdentity, RegistryAdmissionError> {
        if owner != self.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        Ok(self.identity.clone())
    }

    /// Inspect this exact acquisition, including rejected creation awaiting watch retirement.
    /// Absence proves only that this owner retains no record for the supplied binding.
    /// Claim release additionally requires the latched launch outcome and pinned owner.
    pub(crate) fn retained_managed(
        &self,
        resource: ResourceKey,
    ) -> Result<Option<RegistrySnapshot>, RegistryAdmissionError> {
        if resource.owner != self.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        Ok(self.records.iter().flatten().find_map(|record| {
            (record.ticket.binding == FamilyBinding::Managed(resource)).then(|| record.snapshot())
        }))
    }
}
