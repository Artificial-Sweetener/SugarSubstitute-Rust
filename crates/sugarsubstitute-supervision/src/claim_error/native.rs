// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve concrete preadmitted native causes after their final shared controls.
use super::*;
use crate::{
    diagnostic_storage::{
        DiagnosticNativeAllocationFailure, DiagnosticNativeCauseLoan, DiagnosticNativeGrant,
    },
    native_claims::files::{NativeLayoutFailure, NativeRoleLayout},
};
use std::sync::atomic::AtomicUsize;

/// Closed genuine Win32/native allocation leaves; no arbitrary boxed error graph.
pub(crate) enum BoundedNativeCause {
    Windows(windows::core::Error),
    RawIo(std::io::Error),
    Lock(std::fs::TryLockError),
    Reserve(std::collections::TryReserveError),
    Integer(std::num::TryFromIntError),
    Unicode(std::string::FromUtf16Error),
    DiagnosticValidation(crate::diagnostic_storage::DiagnosticRecoveryValidationError),
}
impl BoundedNativeCause {
    /// Preserve the exact concrete ordinary leaf under its existing owner contract.
    pub(crate) fn ordinary_error(self, operation: &'static str) -> ClaimError {
        match self {
            Self::Windows(cause) => ClaimError::new(operation, cause),
            Self::RawIo(cause) => ClaimError::new(operation, cause),
            Self::Lock(cause) => ClaimError::new(operation, cause),
            Self::Reserve(cause) => ClaimError::new(operation, cause),
            Self::Integer(cause) => ClaimError::new(operation, cause),
            Self::Unicode(cause) => ClaimError::new(operation, cause),
            Self::DiagnosticValidation(cause) => ClaimError::new(operation, cause),
        }
    }

    fn source(&self) -> &(dyn Error + 'static) {
        match self {
            Self::Windows(cause) => cause,
            Self::RawIo(cause) => cause,
            Self::Lock(cause) => cause,
            Self::Reserve(cause) => cause,
            Self::Integer(cause) => cause,
            Self::Unicode(cause) => cause,

            Self::DiagnosticValidation(cause) => cause,
        }
    }
}
/// Consume the exact original cause destination before allocating its actual controls.
pub(crate) enum NativeCauseDestination {
    Diagnostic { allocation: DiagnosticNativeCauseLoan, original: DiagnosticNativeGrant },
    Selected(crate::installed_release::CaptureCauseDestination),
}
impl NativeCauseDestination {
    pub(crate) fn diagnostic(
        allocation: DiagnosticNativeCauseLoan,
        original: DiagnosticNativeGrant,
    ) -> Self {
        Self::Diagnostic { allocation, original }
    }
    fn into_ticket(self) -> SharedNativeCauseTicket {
        match self {
            Self::Diagnostic { allocation, original } => SharedNativeCauseTicket {
                data: Some(Arc::new(NativeCauseTicketData::Diagnostic { _allocation: allocation })),
                original: CauseOrigin::Diagnostic { _original: original },
            },
            Self::Selected(destination) => SharedNativeCauseTicket {
                data: Some(Arc::new(NativeCauseTicketData::Selected {
                    _allocation: destination.allocation,
                })),
                original: CauseOrigin::Selected { _original: destination.original },
            },
        }
    }
}
enum NativeCauseTicketData {
    Diagnostic { _allocation: DiagnosticNativeCauseLoan },
    Selected { _allocation: crate::installed_release::SharedCaptureLoan },
}
#[derive(Clone)]
enum CauseOrigin {
    Diagnostic { _original: DiagnosticNativeGrant },
    Selected { _original: crate::installed_release::AllocationOrigin },
}
/// Retain the exact original outside the outer ticket's complete control free.
pub(super) struct SharedNativeCauseTicket {
    data: Option<Arc<NativeCauseTicketData>>,
    original: CauseOrigin,
}
impl Clone for SharedNativeCauseTicket {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), original: self.original.clone() }
    }
}
impl Drop for SharedNativeCauseTicket {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            drop(Arc::into_inner(data));
        }
    }
}
/// Keep the smaller selected leaf control outside its original shared ticket.
pub(super) struct SelectedLeaf {
    data: Option<Arc<crate::installed_release::CaptureBoundedCause>>,
    credit: SharedNativeCauseTicket,
}
impl Clone for SelectedLeaf {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), credit: self.credit.clone() }
    }
}
impl Drop for SelectedLeaf {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            drop(Arc::into_inner(data));
        }
    }
}
impl SelectedLeaf {
    pub(super) fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.data.as_ref()?.source())
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        self.data
            .as_ref()
            .zip(other.data.as_ref())
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }
}
/// Actual leaf storage always deallocates before its outside first-role credit.
pub(super) struct NativeLeaf {
    data: Option<Arc<BoundedNativeCause>>,
    credit: SharedNativeCauseTicket,
}
impl Clone for NativeLeaf {
    fn clone(&self) -> Self {
        Self { data: self.data.clone(), credit: self.credit.clone() }
    }
}
impl Drop for NativeLeaf {
    fn drop(&mut self) {
        if let Some(data) = self.data.take() {
            drop(Arc::into_inner(data));
        }
    }
}
impl NativeLeaf {
    pub(super) fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.data.as_ref()?.source())
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        self.data
            .as_ref()
            .zip(other.data.as_ref())
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }
}
#[repr(C)]
struct ArcStorage<T> {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: T,
}
impl ClaimError {
    /// Assemble a bounded immutable successor using the secondary's funded role.
    pub(crate) fn with_native_secondary(self, secondary: Self) -> Self {
        if self.cleanup_cause().is_some() {
            std::mem::forget(secondary);
            tracing::error!(
                operation = "native_secondary_already_retained",
                "Additional cleanup custody retained"
            );
            return self;
        }
        let Some(credit) = secondary.state.native_ticket().or_else(|| self.state.native_ticket())
        else {
            std::mem::forget(secondary);
            tracing::error!(
                operation = "native_secondary_missing_destination",
                "Unexpected secondary custody retained"
            );
            return self;
        };
        Self { state: ClaimState::Shared(SharedClaimState::joined(self, secondary, Some(credit))) }
    }
    /// Preserve the closed leaf after its destination was admitted before the effect.
    pub(crate) fn native(
        operation: &'static str,
        cause: BoundedNativeCause,
        destination: NativeCauseDestination,
    ) -> Self {
        if matches!(&destination, NativeCauseDestination::Selected(_)) {
            return match crate::installed_release::CaptureBoundedCause::from_native(cause) {
                Ok(cause) => Self::selected_native(operation, cause, destination),
                Err(cause) => {
                    std::mem::forget((cause, destination));
                    Self::capture_allocation(crate::installed_release::CaptureCapacityFailure::new(
                        crate::installed_release::allocation::CapacityFailureKind::Identity,
                    ))
                }
            };
        }
        let credit = destination.into_ticket();
        let leaf = NativeLeaf { data: Some(Arc::new(cause)), credit: credit.clone() };
        Self { state: ClaimState::Shared(SharedClaimState::native(operation, leaf, credit)) }
    }
    /// Preserve full original inline allocation denial without allocating anything.
    pub(crate) fn diagnostic_allocation(cause: DiagnosticNativeAllocationFailure) -> Self {
        Self { state: ClaimState::Allocation(cause) }
    }
    /// Join one secondary under its independently preheld concrete control destination.
    pub(crate) fn finish_native<T>(
        result: Result<T, Self>,
        cleanup: Result<(), Self>,
        destination: NativeCauseDestination,
    ) -> Result<T, Self> {
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
            (Err(cause), Err(secondary)) => {
                if cause.cleanup_cause().is_some() {
                    std::mem::forget((secondary, destination));
                    tracing::error!(
                        operation = "native_secondary_already_retained",
                        "Additional cleanup custody retained"
                    );
                    return Err(cause);
                }
                let credit = destination.into_ticket();
                Err(Self {
                    state: ClaimState::Shared(SharedClaimState::joined(
                        cause,
                        secondary,
                        Some(credit),
                    )),
                })
            }
        }
    }
    /// Source-owned cause destination covers its leaf, ticket and two immutable state controls.
    pub(crate) fn diagnostic_native_cause_layout() -> Result<NativeRoleLayout, NativeLayoutFailure>
    {
        let bytes = size_of::<ArcStorage<BoundedNativeCause>>()
            .checked_add(size_of::<ArcStorage<NativeCauseTicketData>>())
            .and_then(|value| value.checked_add(size_of::<ArcStorage<state::ClaimData>>()))
            .and_then(|value| value.checked_add(size_of::<ArcStorage<state::ClaimData>>()))
            .ok_or(NativeLayoutFailure::Overflow)?;
        Ok(NativeRoleLayout { retained: bytes, peak: bytes })
    }
    /// Secondary destination additionally covers successor state while old aliases live.
    pub(crate) fn diagnostic_secondary_cause_layout()
    -> Result<NativeRoleLayout, NativeLayoutFailure> {
        Self::diagnostic_native_cause_layout()
    }
}
const _: () = assert!(size_of::<crate::diagnostic_storage::DiagnosticStorageError>() <= 120);

impl ClaimError {
    pub(crate) fn capture_allocation(
        cause: crate::installed_release::CaptureCapacityFailure,
    ) -> Self {
        Self { state: ClaimState::CaptureAllocation(cause) }
    }
    pub(crate) fn selected_native(
        operation: &'static str,
        cause: crate::installed_release::CaptureBoundedCause,
        destination: NativeCauseDestination,
    ) -> Self {
        let credit = destination.into_ticket();
        let leaf = SelectedLeaf { data: Some(Arc::new(cause)), credit: credit.clone() };
        Self { state: ClaimState::Shared(SharedClaimState::selected(operation, leaf, credit)) }
    }
    pub(crate) fn capture_native_cause_storage_bytes() -> Option<usize> {
        size_of::<ArcStorage<crate::installed_release::CaptureBoundedCause>>()
            .checked_add(size_of::<ArcStorage<NativeCauseTicketData>>())
            .and_then(|n| {
                n.checked_add(crate::installed_release::SharedCaptureLoan::control_bytes())
            })
            .and_then(|n| n.checked_add(size_of::<ArcStorage<state::ClaimData>>().checked_mul(2)?))
    }
}
