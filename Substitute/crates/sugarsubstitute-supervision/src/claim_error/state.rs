// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep each actual shared cause-state control outside its original allocation credit.
use super::*;
use crate::diagnostic_storage::DiagnosticNativeAllocationFailure;
use native::{NativeLeaf, SharedNativeCauseTicket};

#[derive(Clone)]
pub(super) enum ClaimState {
    Allocation(DiagnosticNativeAllocationFailure),
    CaptureAllocation(crate::installed_release::CaptureCapacityFailure),
    Shared(SharedClaimState),
}
pub(super) enum Primary {
    Ordinary(Arc<dyn Error + Send + Sync>),
    Native(NativeLeaf),
    Selected(super::native::SelectedLeaf),
    Previous(ClaimError),
}
pub(super) struct ClaimData {
    operation: &'static str,
    primary: Primary,
    cleanup: Option<ClaimError>,
}
/// Constructor-private data and inseparable outside credit; no Weak/raw projection.
pub(super) struct SharedClaimState {
    data: Option<Arc<ClaimData>>,
    credit: Option<SharedNativeCauseTicket>,
    storage_code: crate::diagnostic_storage::DiagnosticStorageFailureCode,
    storage_after: Option<crate::diagnostic_storage::DiagnosticStorageFailureCode>,
}
impl Clone for SharedClaimState {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            credit: self.credit.clone(),
            storage_code: self.storage_code,
            storage_after: self.storage_after,
        }
    }
}
impl Drop for SharedClaimState {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            drop(Arc::into_inner(data));
        }
    }
}
impl SharedClaimState {
    pub(super) fn ordinary(operation: &'static str, cause: Arc<dyn Error + Send + Sync>) -> Self {
        Self {
            data: Some(Arc::new(ClaimData {
                operation,
                primary: Primary::Ordinary(cause),
                cleanup: None,
            })),
            credit: None,
            storage_code: crate::diagnostic_storage::DiagnosticStorageFailureCode::Admission,
            storage_after: None,
        }
    }
    pub(super) fn native(
        operation: &'static str,
        leaf: NativeLeaf,
        credit: SharedNativeCauseTicket,
    ) -> Self {
        Self {
            data: Some(Arc::new(ClaimData {
                operation,
                primary: Primary::Native(leaf),
                cleanup: None,
            })),
            credit: Some(credit),
            storage_code: crate::diagnostic_storage::DiagnosticStorageFailureCode::Admission,
            storage_after: None,
        }
    }
    /// Build an immutable successor under the secondary role, retaining predecessor custody.
    pub(super) fn selected(
        operation: &'static str,
        leaf: super::native::SelectedLeaf,
        credit: SharedNativeCauseTicket,
    ) -> Self {
        Self {
            data: Some(Arc::new(ClaimData {
                operation,
                primary: Primary::Selected(leaf),
                cleanup: None,
            })),
            credit: Some(credit),
            storage_code: crate::diagnostic_storage::DiagnosticStorageFailureCode::Admission,
            storage_after: None,
        }
    }
    pub(super) fn joined(
        original: ClaimError,
        secondary: ClaimError,
        credit: Option<SharedNativeCauseTicket>,
    ) -> Self {
        let operation = original.operation();
        let (storage_code, storage_after) = original.state.storage_metadata();
        Self {
            data: Some(Arc::new(ClaimData {
                operation,
                primary: Primary::Previous(original),
                cleanup: Some(secondary),
            })),
            credit,
            storage_code,
            storage_after,
        }
    }
    pub(super) fn ordinary_update(original: ClaimError, secondary: ClaimError) -> Self {
        Self::joined(original, secondary, None)
    }
    pub(super) fn operation(&self) -> &'static str {
        self.data.as_ref().map_or("retired_claim_custody", |data| data.operation)
    }
    pub(super) fn cleanup(&self) -> Option<&ClaimError> {
        self.data.as_ref()?.cleanup.as_ref()
    }
}
impl ClaimState {
    pub(super) const fn storage_metadata(
        &self,
    ) -> (
        crate::diagnostic_storage::DiagnosticStorageFailureCode,
        Option<crate::diagnostic_storage::DiagnosticStorageFailureCode>,
    ) {
        match self {
            Self::Allocation(cause) => (cause.storage_code, cause.storage_after),
            Self::CaptureAllocation(cause) => (cause.storage_code, cause.storage_after),
            Self::Shared(state) => (state.storage_code, state.storage_after),
        }
    }
    pub(super) fn set_storage_metadata(
        &mut self,
        code: crate::diagnostic_storage::DiagnosticStorageFailureCode,
        after: Option<crate::diagnostic_storage::DiagnosticStorageFailureCode>,
    ) {
        match self {
            Self::Allocation(cause) => {
                cause.storage_code = code;
                cause.storage_after = after;
            }
            Self::CaptureAllocation(cause) => {
                cause.storage_code = code;
                cause.storage_after = after;
            }
            Self::Shared(state) => {
                state.storage_code = code;
                state.storage_after = after;
            }
        }
    }
    pub(super) fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Allocation(cause) => Some(cause),
            Self::CaptureAllocation(cause) => Some(cause),
            Self::Shared(state) => match &state.data.as_ref()?.primary {
                Primary::Ordinary(cause) => Some(cause.as_ref()),
                Primary::Native(leaf) => leaf.source(),
                Primary::Selected(leaf) => leaf.source(),
                Primary::Previous(previous) => previous.source(),
            },
        }
    }
    pub(super) fn native_ticket(&self) -> Option<SharedNativeCauseTicket> {
        match self {
            Self::Allocation(_) | Self::CaptureAllocation(_) => None,
            Self::Shared(state) => state.credit.clone(),
        }
    }
    pub(super) fn same_control(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Shared(left), Self::Shared(right)) => left
                .data
                .as_ref()
                .zip(right.data.as_ref())
                .is_some_and(|(left, right)| Arc::ptr_eq(left, right)),
            (Self::Allocation(left), Self::Allocation(right)) => same_allocation(left, right),
            (Self::CaptureAllocation(left), Self::CaptureAllocation(right)) => {
                left.same_native_identity(right)
            }
            _ => false,
        }
    }
    pub(super) fn same_primary(&self, other: &Self) -> bool {
        if let Self::Shared(state) = self
            && let Some(data) = &state.data
            && let Primary::Previous(previous) = &data.primary
        {
            return previous.state.same_primary(other);
        }
        if let Self::Shared(state) = other
            && let Some(data) = &state.data
            && let Primary::Previous(previous) = &data.primary
        {
            return self.same_primary(&previous.state);
        }
        match (self, other) {
            (Self::Allocation(left), Self::Allocation(right)) => same_allocation(left, right),
            (Self::CaptureAllocation(left), Self::CaptureAllocation(right)) => {
                left.same_native_identity(right)
            }
            (Self::Shared(left), Self::Shared(right)) => match (
                left.data.as_ref().map(|data| &data.primary),
                right.data.as_ref().map(|data| &data.primary),
            ) {
                (Some(Primary::Ordinary(left)), Some(Primary::Ordinary(right))) => {
                    Arc::ptr_eq(left, right)
                }
                (Some(Primary::Native(left)), Some(Primary::Native(right))) => left.same(right),
                (Some(Primary::Selected(left)), Some(Primary::Selected(right))) => left.same(right),
                _ => false,
            },
            _ => false,
        }
    }
}
fn same_allocation(
    left: &DiagnosticNativeAllocationFailure,
    right: &DiagnosticNativeAllocationFailure,
) -> bool {
    left.operation() == right.operation()
        && left.code() == right.code()
        && left.post_control_failure() == right.post_control_failure()
}
