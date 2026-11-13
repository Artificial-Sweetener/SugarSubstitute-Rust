// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the concrete Incident callback cause controls before their construction.
use super::*;
use std::{mem::ManuallyDrop, sync::atomic::AtomicUsize};
use sugarsubstitute_contracts::{incident::IncidentError, lifecycle::IdentityError};

#[repr(C)]
struct ArcStorage<T> {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: T,
}
#[repr(C)]
struct CodecHandleEnvelope {
    original_serde_box: usize,
    original_decode: DiagnosticRecoveryDecodeErrorLoan,
}
#[repr(C)]
union AllocationCustodyEnvelope {
    collection: ManuallyDrop<DiagnosticRecoveryCollectionErrorLoan>,
    pending: ManuallyDrop<DiagnosticPendingPublicationLoan>,
}
#[repr(C)]
struct AllocationFailureEnvelope {
    original_try_reserve: std::collections::TryReserveError,
    custody_tag: u32,
    original_custody: AllocationCustodyEnvelope,
}
#[repr(C)]
union IncidentPayloadEnvelope {
    codec: ManuallyDrop<CodecHandleEnvelope>,
    allocation: ManuallyDrop<AllocationFailureEnvelope>,
    facts: IncidentError,
    identity: IdentityError,
    storage: ManuallyDrop<DiagnosticStorageError>,
    validated: ManuallyDrop<DiagnosticRecoveryValidationError>,
    schema: u32,
}
#[repr(C)]
struct IncidentStoreErrorEnvelope {
    tag: u32,
    payload: IncidentPayloadEnvelope,
}
#[repr(C)]
struct OuterValidationEnvelope {
    source: DiagnosticRecoveryValidationError,
    original_decode: DiagnosticRecoveryDecodeErrorLoan,
}

/// Fixed inline destination for the actual Incident semantic cause, in bytes.
/// Concrete Incident producers must establish size and alignment inequalities.
pub const DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_BYTES: usize =
    size_of::<IncidentStoreErrorEnvelope>();
/// Required alignment of the fixed Incident cause destination, in bytes.
pub const DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_ALIGNMENT: usize =
    align_of::<IncidentStoreErrorEnvelope>();
/// Fixed inline destination for the actual outside Decode error wrapper, in bytes.
pub const DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_BYTES: usize = size_of::<OuterValidationEnvelope>();
/// Required alignment of the fixed outside Decode error wrapper, in bytes.
pub const DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_ALIGNMENT: usize =
    align_of::<OuterValidationEnvelope>();

pub(super) const fn inner_destination_bytes() -> usize {
    size_of::<ArcStorage<IncidentStoreErrorEnvelope>>()
        + size_of::<ArcStorage<IncidentValidationCredit>>()
}
pub(super) const fn outer_destination_bytes() -> usize {
    size_of::<ArcStorage<OuterValidationEnvelope>>()
        + size_of::<ArcStorage<IncidentValidationCredit>>()
}

/// Own the two distinct preadmitted callback controls under the original Decode.
/// This value accepts no caller type, size, layout, claim or native permission.
pub struct DiagnosticIncidentValidationControls {
    inner: DiagnosticIncidentValidationCauseLoan,
    outer: DiagnosticIncidentValidationCauseLoan,
}
impl DiagnosticIncidentValidationControls {
    /// Transfer the one-use inner and outer slots without allocating.
    pub fn into_parts(
        self,
    ) -> (DiagnosticIncidentValidationCauseLoan, DiagnosticIncidentValidationCauseLoan) {
        (self.inner, self.outer)
    }
}
/// Keep one actual source control charged until its final physical destruction.
/// Consumers cannot clone this loan or acquire another allocation through it.
pub struct DiagnosticIncidentValidationCauseLoan {
    credit: Option<Arc<IncidentValidationCredit>>,
    original: DiagnosticAllocationOwner,
}
struct IncidentValidationCredit {
    _allocation: DiagnosticAllocationLoan,
}
impl DiagnosticIncidentValidationCauseLoan {
    fn adopt(allocation: DiagnosticAllocationLoan) -> Self {
        let original = allocation.owner.clone();
        Self {
            credit: Some(Arc::new(IncidentValidationCredit { _allocation: allocation })),
            original,
        }
    }
    pub(in crate::diagnostic_storage) fn clone_alias(&self) -> Self {
        Self { credit: self.credit.clone(), original: self.original.clone() }
    }
}
impl Drop for DiagnosticIncidentValidationCauseLoan {
    fn drop(&mut self) {
        if let Some(credit) = self.credit.take() {
            // Every alias consumes into_inner; its control frees before credit returns.
            drop(Arc::into_inner(credit));
        }
    }
}
struct AdmissionSampling {
    original: DiagnosticAllocationOwner,
    pending: bool,
}
impl Drop for AdmissionSampling {
    fn drop(&mut self) {
        if self.pending {
            self.original.close();
        }
    }
}
impl DiagnosticRecoveryDecodeLoan {
    /// Admit both fixed controls before parsing or constructing an Incident cause.
    /// The original retained Decode binding is sampled outside allocation locks;
    /// its already narrowed deadline is preserved. This grants no native access.
    /// # Errors
    /// Rejects another issue, closed/expired/cancelled original control, or capacity.
    pub fn incident_validation_controls(
        &mut self,
    ) -> Result<DiagnosticIncidentValidationControls, DiagnosticStorageError> {
        if self.validation_issued {
            return Err(DiagnosticStorageError::rejected(None, Code::Capacity));
        }
        self.validation_issued = true;
        let original = self
            .original
            .as_ref()
            .ok_or_else(|| DiagnosticStorageError::rejected(None, Code::Closed))?;
        let snapshot =
            original.binding.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        let owner = &original._phase.owner;
        let mut sampling = AdmissionSampling { original: owner.clone(), pending: true };
        let observation = snapshot.observe_original();
        sampling.pending = false;
        {
            let mut binding =
                original.binding.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            binding.validate(owner, observation)?;
        }
        // Both destinations exist before either shared credit control is allocated.
        let inner = owner.residual(roles::ResidualRole::IncidentValidationInner, snapshot.key())?;
        let outer = owner.residual(roles::ResidualRole::IncidentValidationOuter, snapshot.key())?;
        Ok(DiagnosticIncidentValidationControls {
            inner: DiagnosticIncidentValidationCauseLoan::adopt(inner),
            outer: DiagnosticIncidentValidationCauseLoan::adopt(outer),
        })
    }
}
