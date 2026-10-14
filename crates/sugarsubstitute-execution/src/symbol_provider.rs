// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Seal the original Provider1MiB/4 share for actual symbol-provider effects and cleanup.
use crate::ExecutionClient;
use crate::symbol_bundle::{SymbolBundleOrigin, SymbolBundleRetirement, purpose::Purpose};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, Moment, NativeHandleOutput,
};
/// Consume the sole provider share from the actual paired running main.
pub struct SymbolProviderAdmission {
    origin: SymbolProviderOrigin,
}
impl SymbolProviderAdmission {
    /// Transfer identical original custody without allocation or renewed authority.
    pub fn into_origin(self) -> SymbolProviderOrigin {
        self.origin
    }
}
/// Closed original provider custody. Clones retain the same private control.
/// The shared no-Weak control frees before its actual1MiB claim; no raw claim escapes.
#[derive(Clone)]
pub struct SymbolProviderOrigin(SymbolBundleOrigin);
impl SymbolProviderOrigin {
    /// Borrow actual reconstructed Core ports; no caller clock/cancellation is retained.
    /// # Errors
    /// Returns invalid admission if the closed original owner is unavailable.
    pub fn original_context(&self) -> Result<&JobContext, AdmissionError> {
        self.0.original_context()
    }
    /// Validate the original actual running main/ledger/current scope cutoffs before effects.
    /// # Errors
    /// Preserves original cancellation, admission, deadline and clock failures.
    pub fn check_effect(&self) -> Result<(), AdmissionError> {
        self.0.check_effect()
    }
    /// Actual single original control size, included in the provider's fixed metadata.
    pub const fn control_storage_bytes(&self) -> usize {
        self.0.control_storage_bytes()
    }
    /// Compare only the identical original allocation; this grants no OS authority.
    pub fn same_origin(&self, other: &Self) -> bool {
        self.0.same_origin(&other.0)
    }
    /// Original provider claim in bytes, unchanged for every alias.
    pub const fn retained_bytes(&self) -> u64 {
        1_048_576
    }
    /// Reserve one of the actual Provider4 outputs after original validation.
    /// # Errors
    /// Preserves real original controls and the fixed partition's actual capacity error.
    pub fn reserve_output(&self) -> Result<NativeHandleOutput, AdmissionError> {
        self.0.reserve_output()
    }
}
/// Close-only authority from the actual same activated pre-held cleanup job.
pub struct SymbolProviderRetirement(SymbolBundleRetirement);
impl SymbolProviderRetirement {
    /// Check real cleanup controls and its first immutable cutoff before fresh cleanup.
    /// # Errors
    /// Preserves actual cleanup cancellation, deadline and original clock failures.
    pub fn check(&self) -> Result<(), AdmissionError> {
        self.0.check()
    }
    /// Borrow the reconstructed actual original cleanup context.
    pub const fn original_context(&self) -> &JobContext {
        self.0.original_context()
    }
    /// First immutable cleanup cutoff; this proof never renews it.
    pub const fn cleanup_cutoff(&self) -> Moment {
        self.0.cleanup_cutoff()
    }
    /// Compare identical original provider custody without asserting native settlement.
    pub fn matches_origin(&self, target: &SymbolProviderOrigin) -> bool {
        self.0.matches_origin(&target.0)
    }
}
impl ExecutionClient {
    /// Issue once the genuine1MiB/4 provider share of a running paired main.
    /// Matching context facts are validated against actual memory/native allocations;
    /// Core clock/cancellation are reconstructed, and providers run outside owner locks.
    /// # Errors
    /// Rejects foreign/counterfeit records, prior issuance, claim/partition failures and
    /// actual cancellation/deadline/clock failure without issuing caller-sized authority.
    pub fn symbol_provider_admission(
        &self,
        observed: &JobContext,
    ) -> Result<SymbolProviderAdmission, AdmissionError> {
        Ok(SymbolProviderAdmission {
            origin: SymbolProviderOrigin(crate::symbol_bundle::admission::admit(
                self,
                observed,
                Purpose::Provider,
            )?),
        })
    }
    /// Derive provider close-only proof from the genuine paired activated cleanup record.
    /// Target main may be terminal/acknowledged; original custody remains retained.
    /// # Errors
    /// Rejects unrelated/unactivated cleanup and preserves its original controls/cutoff.
    pub fn symbol_provider_retirement(
        &self,
        target: &SymbolProviderOrigin,
        observed: &JobContext,
    ) -> Result<SymbolProviderRetirement, AdmissionError> {
        Ok(SymbolProviderRetirement(crate::symbol_bundle::retirement::retire(
            self, &target.0, observed,
        )?))
    }
}

#[cfg(test)]
#[path = "symbol_provider/tests.rs"]
mod tests;
