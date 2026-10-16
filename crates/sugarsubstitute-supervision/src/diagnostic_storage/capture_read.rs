// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own genuine capture generation directory leases under the same finite store operation.
mod retirement;
use super::*;
use crate::protected_storage::capture_read::CaptureReadOwner;
use std::os::windows::io::BorrowedHandle;
/// Retain actual duplicated database/metrics handles, source pin and both original credits.
/// Dropping uncertain native custody retains the complete owner without closing handles.
pub struct ProtectedCaptureReadLease {
    owner: CaptureReadOwner,
    shared: Option<Shared>,
    operation: LogicalStoreOperationKey,
    reserved_handles: usize,
}
/// Borrow only the same actual directory pins during one synchronous checked ABI call.
/// The genuine current context and guard cannot be retained in a native token.
pub struct ProtectedCaptureReadGuard<'call> {
    lease: &'call ProtectedCaptureReadLease,
    current: &'call JobContext,
}
impl DiagnosticStorage {
    /// Admit charged output before duplicating the actual protected generation pins.
    /// Source access occurs through the capture owner's exclusive native loan.
    /// # Errors
    /// Rejects occupied output, wrong source/Core/namespace/current operation or bounded
    /// read/handle credit. After admission, partial failure keeps the same output Some.
    pub fn prepare_capture_read(
        &mut self,
        source: &mut crate::ProtectedStorageRoot,
        output: &mut Option<ProtectedCaptureReadLease>,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<(), DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        if output.is_some()
            || state.data_mut()?.reads.len() + state.data_mut()?.capture_reads
                >= DIAGNOSTIC_READ_BOUND
            || state
                .data_mut()?
                .live_handles()
                .checked_add(2)
                .is_none_or(|count| count > DIAGNOSTIC_HANDLE_BOUND)
        {
            return Err(error(operation.key, Code::Capacity));
        }
        let original = state.data_mut()?.allocation.native_grant();
        drop(state);
        let owner = source
            .admit_capture_read(original, operation.key.owner().namespace, context)
            .map_err(|cause| native(operation.key, Code::Admission, cause))?;
        let mut state = self.check(operation, context)?;
        if state.data_mut()?.reads.len() + state.data_mut()?.capture_reads >= DIAGNOSTIC_READ_BOUND
            || state
                .data_mut()?
                .live_handles()
                .checked_add(2)
                .is_none_or(|count| count > DIAGNOSTIC_HANDLE_BOUND)
        {
            return Err(error(operation.key, Code::Capacity));
        }
        state.data_mut()?.capture_reads += 1;
        state.data_mut()?.capture_handles += 2;
        *output = Some(ProtectedCaptureReadLease {
            owner,
            shared: Some(self.shared.clone()),
            operation: operation.key,
            reserved_handles: 2,
        });
        drop(state);
        let lease = output.as_mut().ok_or_else(|| error(operation.key, Code::Closed))?;
        source
            .prepare_capture_read_handles(&mut lease.owner, context)
            .map_err(|cause| native(operation.key, Code::Read, cause))?;
        let state = self.check(operation, context)?;
        drop(state);
        Ok(())
    }
}
impl ProtectedCaptureReadLease {
    /// Observe full matching origin from the actual retained source, never another capability.
    pub fn origin(
        &self,
    ) -> Result<sugarsubstitute_contracts::crash::CaptureGenerationOrigin, DiagnosticStorageError>
    {
        self.owner
            .data
            .as_ref()
            .map(|data| data.origin)
            .ok_or_else(|| error(self.operation, Code::Closed))
    }
    pub(super) fn matches_storage(
        &self,
        shared: &Shared,
        operation: LogicalStoreOperationKey,
    ) -> bool {
        self.operation == operation
            && self.shared.as_ref().is_some_and(|source| Shared::ptr_eq(source, shared))
    }
    /// Check current controls and actual native identity/protection before an ABI call.
    /// # Errors
    /// Rejects source close, foreign/revoked original operation and partial/changed pins.
    /// Native work and provider observations run outside the storage mutex.
    pub fn borrow<'call>(
        &'call mut self,
        current: &'call JobContext,
    ) -> Result<ProtectedCaptureReadGuard<'call>, DiagnosticStorageError> {
        let shared = self.shared.as_ref().ok_or_else(|| error(self.operation, Code::Closed))?;
        let mut state = StateAccess::acquire(shared, self.operation);
        state.check(self.operation, current, false)?;
        drop(state);
        self.owner
            .validate_handles(current)
            .map_err(|cause| native(self.operation, Code::Read, cause))?;
        Ok(ProtectedCaptureReadGuard { lease: self, current })
    }
}
impl ProtectedCaptureReadGuard<'_> {
    /// Borrow the immutable genuine current context for ephemeral effect callbacks.
    pub fn context(&self) -> &JobContext {
        self.current
    }
    /// Observe actual original controls immediately before each native effect callback.
    /// # Errors
    /// Rejects a stopped current operation or closed source; no native work occurs.
    pub fn check_current(&self) -> Result<(), DiagnosticStorageError> {
        self.lease
            .owner
            .current(self.current)
            .map_err(|cause| native(self.lease.operation, Code::Closed, cause))
    }
    /// Observe the exact genuine current storage operation, independent of historical origin.
    pub const fn current_operation(&self) -> LogicalStoreOperationKey {
        self.lease.operation
    }
    /// Observe original generation ownership without rebasing historical identity.
    pub fn coordinator(&self) -> Result<CoordinatorKey, DiagnosticStorageError> {
        self.lease
            .owner
            .data
            .as_ref()
            .map(|data| data.coordinator)
            .ok_or_else(|| error(self.lease.operation, Code::Closed))
    }
    /// Return exact origin matching facts retained by the real protected source pin.
    /// This does not construct or authenticate another generation/source capability.
    pub fn origin(
        &self,
    ) -> Result<sugarsubstitute_contracts::crash::CaptureGenerationOrigin, DiagnosticStorageError>
    {
        self.lease.origin()
    }
    /// Borrow the actual database directory only for the synchronous native ABI call.
    /// # Errors
    /// Rejects a retired/partial pin. The borrowed handle must never be closed/retained.
    pub fn database(&self) -> Result<BorrowedHandle<'_>, DiagnosticStorageError> {
        self.handle(0)
    }
    /// Borrow the actual metrics directory only for the synchronous native ABI call.
    /// # Errors
    /// Rejects a retired/partial pin. The borrowed handle must never be closed/retained.
    pub fn metrics(&self) -> Result<BorrowedHandle<'_>, DiagnosticStorageError> {
        self.handle(1)
    }
    /// Observe full actual directory identities as inert correlation data.
    pub fn identities(&self) -> Result<[[u8; 24]; 2], DiagnosticStorageError> {
        self.lease
            .owner
            .data
            .as_ref()
            .map(|data| data.identities)
            .ok_or_else(|| error(self.lease.operation, Code::Closed))
    }
    fn handle(&self, index: usize) -> Result<BorrowedHandle<'_>, DiagnosticStorageError> {
        self.check_current()?;
        self.lease
            .owner
            .borrowed(index)
            .map_err(|cause| native(self.lease.operation, Code::Read, cause))
    }
}
impl Drop for ProtectedCaptureReadLease {
    fn drop(&mut self) {
        if self.owner.held() {
            if let Some(shared) = self.shared.take() {
                std::mem::forget(shared);
            }
        } else if let Some(shared) = self.shared.take() {
            let mut state = lock(&shared);
            match state.capture_reads.checked_sub(1) {
                Some(count) => {
                    state.capture_reads = count;
                    if let Some(handles) = state.capture_handles.checked_sub(self.reserved_handles)
                    {
                        state.capture_handles = handles;
                    } else {
                        state.closed = true;
                        state.allocation.close();
                        drop(state);
                        std::mem::forget(shared);
                        tracing::error!(
                            operation = "capture_handle_slot_retained",
                            "Original capture handle accounting retained"
                        );
                        return;
                    }
                    drop(state);
                    drop(shared);
                }
                None => {
                    state.closed = true;
                    state.capture_reads = 1;
                    state.allocation.close();
                    drop(state);
                    std::mem::forget(shared);
                    tracing::error!(
                        operation = "capture_read_slot_retained",
                        "Capture read slot custody retained"
                    );
                }
            }
        }
    }
}

mod recovery;

pub(super) mod origin;
pub(super) use origin::recognized_origin;
pub use origin::{
    CheckedRecoveryCaptureOrigin, RecordedRecoveryCaptureOrigin, RecoveryCaptureOriginValidator,
};

mod historical;

mod promotion;
pub use promotion::{CapturePromotionValidator, RecordedCapturePromotion};
