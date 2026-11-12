// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Seal the distinct committed-reader3MiB/140 share without exposing ledger projections.
use super::{SymbolBundleOrigin, SymbolBundleRetirement, purpose::Purpose};
use crate::ExecutionClient;
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobContext, Moment, NativeHandleOutput,
};
/// Consume the sole committed-reader share from the actual paired running main.
pub struct CommittedReaderAdmission {
    origin: CommittedReaderOrigin,
}
impl CommittedReaderAdmission {
    /// Transfer identical original custody without allocation or renewed authority.
    pub fn into_origin(self) -> CommittedReaderOrigin {
        self.origin
    }
}
/// Closed original committed-reader custody. Clones retain the same private control.
/// The shared no-Weak control frees before its actual3MiB claim; no raw claim escapes.
#[derive(Clone)]
pub struct CommittedReaderOrigin(SymbolBundleOrigin);
impl CommittedReaderOrigin {
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
    /// Actual single original control size, included in the reader's fixed metadata.
    pub const fn control_storage_bytes(&self) -> usize {
        self.0.control_storage_bytes()
    }
    /// Compare only the identical original allocation; this grants no OS authority.
    pub fn same_origin(&self, other: &Self) -> bool {
        self.0.same_origin(&other.0)
    }
    /// Original committed-reader claim in bytes, unchanged for every alias.
    pub const fn retained_bytes(&self) -> u64 {
        3_145_728
    }
    /// Reserve one of the actual committed-reader140 outputs after original validation.
    /// # Errors
    /// Preserves real original controls and the fixed partition's actual capacity error.
    pub fn reserve_output(&self) -> Result<NativeHandleOutput, AdmissionError> {
        self.0.reserve_output()
    }
}
/// Close-only authority from the actual same activated pre-held cleanup job.
pub struct CommittedReaderRetirement(SymbolBundleRetirement);
impl CommittedReaderRetirement {
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
    /// Compare identical original reader custody without asserting native settlement.
    pub fn matches_origin(&self, target: &CommittedReaderOrigin) -> bool {
        self.0.matches_origin(&target.0)
    }
}
impl ExecutionClient {
    /// Issue once the genuine3MiB/140 committed-reader share of a running paired main.
    /// Matching context facts are validated against actual memory/native allocations;
    /// Core clock/cancellation are reconstructed, and providers run outside owner locks.
    /// # Errors
    /// Rejects foreign/counterfeit records, prior issuance, claim/partition failures and
    /// actual cancellation/deadline/clock failure without issuing caller-sized authority.
    pub fn committed_reader_admission(
        &self,
        observed: &JobContext,
    ) -> Result<CommittedReaderAdmission, AdmissionError> {
        Ok(CommittedReaderAdmission {
            origin: CommittedReaderOrigin(super::admission::admit(
                self,
                observed,
                Purpose::CommittedReader,
            )?),
        })
    }
    /// Derive reader close-only proof from the genuine paired activated cleanup record.
    /// Target main may be terminal/acknowledged; original custody remains retained.
    /// # Errors
    /// Rejects unrelated/unactivated cleanup and preserves its original controls/cutoff.
    pub fn committed_reader_retirement(
        &self,
        target: &CommittedReaderOrigin,
        observed: &JobContext,
    ) -> Result<CommittedReaderRetirement, AdmissionError> {
        Ok(CommittedReaderRetirement(super::retirement::retire(self, &target.0, observed)?))
    }
}
