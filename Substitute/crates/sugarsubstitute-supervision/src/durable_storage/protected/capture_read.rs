// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual generation source pins independently of current finite reader work.
#[cfg(any(test, feature = "boundary-fixtures"))]
pub(crate) mod fixture;
mod native;
pub(crate) mod pin;
mod recovery;
use super::*;
use pin::ReadPin;
pub(crate) struct CaptureReadData {
    pub(crate) handles: [Handle; 2],
    pub(crate) coordinator: CoordinatorKey,
    pub(crate) origin: CaptureGenerationOrigin,
    pub(crate) identities: [[u8; 24]; 2],
    pub(crate) ready: bool,
    pub(crate) recovery_handles: Vec<Handle>,
    sid: [u8; 72],
    sid_length: usize,
    causes: NativeCauseDestinations,
    pub(crate) original: DiagnosticNativeGrant,
    pin: ReadPin,
}
pub(crate) struct CaptureReadOwner {
    pub(crate) data: Option<Box<CaptureReadData>>,
    allocation: Option<DiagnosticNativeOwnerLoan>,
}
impl CaptureReadOwner {
    pub(crate) fn layout() -> Result<files::NativeRoleLayout, files::NativeLayoutFailure> {
        let bytes = size_of::<CaptureReadData>() + 5 * size_of::<Handle>();
        Ok(files::NativeRoleLayout { retained: bytes, peak: bytes })
    }
    pub(crate) fn held(&self) -> bool {
        self.data.as_ref().is_some_and(|data| {
            data.handles.iter().chain(data.recovery_handles.iter()).any(Handle::is_armed)
        })
    }
    pub(crate) fn current(&self, context: &JobContext) -> Result<(), ClaimError> {
        let data = self.data.as_ref().ok_or_else(|| ClaimError::invalid("capture_read_closed"))?;
        data.original.check(context).map_err(ClaimError::diagnostic_allocation)?;
        if !data.pin.open() {
            return Err(ClaimError::diagnostic_allocation(data.original.reject_native_state()));
        }
        Ok(())
    }
}
impl Drop for CaptureReadOwner {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            if data.handles.iter().chain(data.recovery_handles.iter()).any(Handle::is_armed) {
                std::mem::forget(data);
                if let Some(allocation) = self.allocation.take() {
                    std::mem::forget(allocation);
                }
            } else {
                drop(data);
            }
        }
    }
}
impl ProtectedStorageRoot {
    pub(crate) fn admit_capture_read(
        &self,
        original: DiagnosticNativeGrant,
        namespace: sugarsubstitute_contracts::lifecycle::NamespaceId,
        context: &JobContext,
    ) -> Result<CaptureReadOwner, ClaimError> {
        original.check(context).map_err(ClaimError::diagnostic_allocation)?;
        let state =
            self.state.as_ref().ok_or_else(|| ClaimError::invalid("capture_read_source_closed"))?;
        let generation = state
            .generation
            .as_ref()
            .filter(|generation| generation.ready)
            .ok_or_else(|| state.budget.rejection("capture_read_generation_unavailable"))?;
        if !state.budget.is_ordinary()
            || state.execution != Some(context.owner_id())
            || generation.coordinator.namespace != namespace
            || state.close_deadline.is_some()
        {
            return Err(state.budget.rejection("capture_read_source_identity"));
        }
        let sid = state.sid.bytes()?;
        if sid.len() > 72 {
            return Err(state.budget.rejection("capture_read_sid_bound"));
        }
        let allocation =
            original.capture_read_owner().map_err(ClaimError::diagnostic_allocation)?;
        let causes =
            NativeCauseDestinations::admit(&original).map_err(ClaimError::diagnostic_allocation)?;
        let pin = state.readers.acquire()?;
        original
            .bind_capture_source(pin.fence(), context)
            .map_err(ClaimError::diagnostic_allocation)?;
        let mut copied_sid = [0; 72];
        copied_sid[..sid.len()].copy_from_slice(sid);
        let budget = Budget::Diagnostic(original.clone());
        Ok(CaptureReadOwner {
            data: Some(Box::new(CaptureReadData {
                handles: [
                    Handle::new(windows::Win32::Foundation::HANDLE::default(), budget.clone()),
                    Handle::new(windows::Win32::Foundation::HANDLE::default(), budget),
                ],
                coordinator: generation.coordinator,
                origin: generation.origin,
                identities: generation.identities,
                ready: false,
                recovery_handles: Vec::new(),
                sid: copied_sid,
                sid_length: sid.len(),
                causes,
                original,
                pin,
            })),
            allocation: Some(allocation),
        })
    }
}

impl ProtectedStorageRoot {
    pub(crate) fn capture_origin(&self) -> Option<CaptureGenerationOrigin> {
        let state = self.state.as_ref()?;
        let generation = state.generation.as_ref().filter(|generation| generation.ready)?;
        Some(generation.origin)
    }
}
