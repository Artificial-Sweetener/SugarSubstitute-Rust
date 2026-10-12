// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind claim retirement to the actual registry owner and exact managed generation.

use super::*;
use sugarsubstitute_contracts::lifecycle::ResourceKey;

/// Retain one opaque registry lifetime; matching coordinator facts cannot recreate it.
/// Every clone consumes its Arc on drop; no raw Arc or Weak projection is exposed.
/// The final allocation is freed before its original noncloneable claim is returned.
#[derive(Clone)]
pub(crate) struct RegistryIdentity(Option<Arc<TableReservation>>);

/// Keep the original admitted table allocation charged through every custody pin.
struct TableReservation {
    _claim: sugarsubstitute_contracts::execution::MemoryClaim,
}

impl RegistryIdentity {
    /// Transfer an already acquired actual table claim to its shared lifetime pin.
    pub(super) fn new(claim: sugarsubstitute_contracts::execution::MemoryClaim) -> Self {
        Self(Some(Arc::new(TableReservation { _claim: claim })))
    }

    /// Account for the retained payload and the Arc strong/weak counter allocation.
    pub(super) const fn allocation_bytes() -> usize {
        size_of::<TableReservation>() + 2 * size_of::<usize>()
    }

    /// Compare actual retained owners without exposing their allocation or native state.
    pub(crate) fn same_as(&self, other: &Self) -> bool {
        self.0.as_ref().zip(other.0.as_ref()).is_some_and(|(a, b)| Arc::ptr_eq(a, b))
    }
}

impl Drop for RegistryIdentity {
    /// Move the last claimed payload out after its private no-Weak allocation is freed.
    fn drop(&mut self) {
        if let Some(owner) = self.0.take() {
            drop(Arc::into_inner(owner));
        }
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
