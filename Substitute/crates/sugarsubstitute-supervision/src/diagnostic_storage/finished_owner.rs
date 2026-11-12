// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve the source-declared native Finished owner before any C++ effect.
use super::*;

/// Source-declared C++ storage envelope, including its 512KiB synchronous temporary.
/// The pinned native header asserts its actual retained fields fit this envelope.
/// Rust bridge buffers and opaque OS/CRT/CNG allocations are separately accounted.
pub const FINISHED_OWNER_STORAGE_BYTES: usize = 4 * 1024 * 1024;
/// Source-declared Rust token/control/output destination, separately from C++ storage.
pub const FINISHED_RUST_STORAGE_BYTES: usize = 16 * 1024;
pub(super) const FINISHED_NATIVE_OUTPUTS: usize = 11;
pub(super) const FINISHED_METADATA_STAGES: usize = 2;

/// Retain a closed memory/output reservation under the original store operation.
/// This type grants no token, source selection, preparation or settlement authority.
/// The actual native adapter keeps it after native data and preserves the complete
/// pair on uncertainty. Drop observes no provider and performs no native call.
pub struct ProtectedFinishedOwnerAllocation {
    shared: Option<Shared>,
    operation: LogicalStoreOperationKey,
    id: u64,
    allocation: Option<DiagnosticNativeOwnerLoan>,
}
/// Keep Rust token state and original causes funded through source closure.
/// This owner grants no C++ effect or settlement authority and holds no native
/// output/stage marker. Actual data/control must be destroyed before this guard.
pub struct ProtectedFinishedRustRetention {
    operation: LogicalStoreOperationKey,
    _allocation: DiagnosticNativeOwnerLoan,
}
impl ProtectedFinishedRustRetention {
    /// Observe the complete original binding without projecting native authority.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Match the actual original allocation pair; keys alone cannot establish this.
    /// This comparison grants no native effect or retirement authority.
    pub fn matches_cpp(&self, cpp: &ProtectedFinishedOwnerAllocation) -> bool {
        self.operation == cpp.operation
            && cpp.allocation.as_ref().is_some_and(|loan| self._allocation.matches_loan(loan))
    }
}
impl DiagnosticStorage {
    /// Reserve separate C++ and Rust owners before entering the native ABI.
    /// Actual adapters retain the C++ guard until unused or checked native retirement,
    /// and retain the Rust guard after all token/control/cause data until deallocation.
    /// # Errors
    /// Rejects foreign/stopped source or operation, occupied owner, insufficient
    /// original residual bytes, 260-output capacity or eight-stage capacity.
    /// Rejection preserves the generation lease and grants no native effect.
    pub fn reserve_finished_owner(
        &mut self,
        generation: &mut ProtectedCaptureReadLease,
        operation: &DiagnosticStorageOperation,
        current: &JobContext,
    ) -> Result<
        (ProtectedFinishedOwnerAllocation, ProtectedFinishedRustRetention),
        DiagnosticStorageError,
    > {
        let mut state = self.check(operation, current)?;
        if !generation.matches_storage(&self.shared, operation.key) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        preflight(state.data_mut()?, operation.key)?;
        let original = state.data_mut()?.allocation.native_grant();
        drop(state);
        generation.borrow(current)?;
        let allocation = original.finished_owner(current).map_err(|cause| {
            native(operation.key, Code::Capacity, ClaimError::diagnostic_allocation(cause))
        })?;
        let rust_allocation = original.finished_rust_owner(current).map_err(|cause| {
            native(operation.key, Code::Capacity, ClaimError::diagnostic_allocation(cause))
        })?;
        let mut state = self.check(operation, current)?;
        preflight(state.data_mut()?, operation.key)?;
        let id = next_lease(&mut state, operation.key)?;
        state.data_mut()?.finished_owner = Some(id);
        drop(state);
        let cpp = ProtectedFinishedOwnerAllocation {
            shared: Some(self.shared.clone()),
            operation: operation.key,
            id,
            allocation: Some(allocation),
        };
        let rust = ProtectedFinishedRustRetention {
            operation: operation.key,
            _allocation: rust_allocation,
        };
        Ok((cpp, rust))
    }
}
fn preflight(
    state: &State,
    operation: LogicalStoreOperationKey,
) -> Result<(), DiagnosticStorageError> {
    if state.finished_owner.is_some()
        || state.genesis.as_ref().is_some_and(|genesis| !genesis.committed)
        || state
            .stage_slots()
            .checked_add(FINISHED_METADATA_STAGES)
            .is_none_or(|n| n > DIAGNOSTIC_STAGE_BOUND)
        || state
            .live_handles()
            .checked_add(FINISHED_NATIVE_OUTPUTS)
            .is_none_or(|n| n > DIAGNOSTIC_HANDLE_BOUND)
    {
        return Err(error(operation, Code::Capacity));
    }
    Ok(())
}
impl ProtectedFinishedOwnerAllocation {
    /// Observe the complete original binding without projecting native authority.
    pub const fn operation(&self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Match the actual retained source's store and full original operation.
    /// This comparison grants no source, native effect or settlement authority.
    pub fn matches_generation(&self, generation: &ProtectedCaptureReadLease) -> bool {
        self.shared
            .as_ref()
            .is_some_and(|shared| generation.matches_storage(shared, self.operation))
    }
}
impl Drop for ProtectedFinishedOwnerAllocation {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.take() {
            let mut state = lock(&shared);
            if state.finished_owner == Some(self.id) {
                state.finished_owner = None;
                drop(state);
                drop(shared);
            } else {
                state.closed = true;
                state.allocation.close();
                drop(state);
                std::mem::forget(shared);
                if let Some(allocation) = self.allocation.take() {
                    std::mem::forget(allocation);
                }
                tracing::error!(
                    operation = "finished_owner_credit_retained",
                    "Finished owner credit retained"
                );
            }
        }
    }
}
