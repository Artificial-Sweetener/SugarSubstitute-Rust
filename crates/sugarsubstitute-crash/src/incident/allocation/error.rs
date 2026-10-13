// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain admitted allocation custody through genuine parser and reserve failures.
use super::super::IncidentStoreError;
use std::{error::Error, fmt, sync::Arc};
use sugarsubstitute_contracts::incident::IncidentError;
use sugarsubstitute_supervision::diagnostic_storage::{
    DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_ALIGNMENT, DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_BYTES,
    DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_ALIGNMENT, DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_BYTES,
    DiagnosticIncidentValidationCause, DiagnosticIncidentValidationCauseLoan,
    DiagnosticPendingPublicationLoan, DiagnosticRecoveryCollectionErrorLoan,
    DiagnosticRecoveryCollectionLoan, DiagnosticRecoveryDecodeErrorLoan,
    DiagnosticRecoveryDecodeLoan, DiagnosticRecoveryValidationError, DiagnosticStorageError,
};

/// An original reserve failure retaining the already admitted destination.
/// The actual cause drops before its collection or pending-publication custody.
pub struct LoanedAllocationFailure {
    source: std::collections::TryReserveError,
    _custody: AllocationCustody,
}
enum AllocationCustody {
    Collection { _loan: DiagnosticRecoveryCollectionErrorLoan },
    Pending { _loan: DiagnosticPendingPublicationLoan },
}
impl LoanedAllocationFailure {
    pub(in crate::incident) fn collection(
        source: std::collections::TryReserveError,
        custody: DiagnosticRecoveryCollectionLoan,
    ) -> Self {
        Self { source, _custody: AllocationCustody::Collection { _loan: custody.into_error() } }
    }
    pub(in crate::incident) fn pending(
        source: std::collections::TryReserveError,
        custody: DiagnosticPendingPublicationLoan,
    ) -> Self {
        Self { source, _custody: AllocationCustody::Pending { _loan: custody } }
    }
    /// Borrow the genuine standard-library allocation cause.
    pub const fn reserve_error(&self) -> &std::collections::TryReserveError {
        &self.source
    }
}
impl fmt::Debug for LoanedAllocationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoanedAllocationFailure").finish_non_exhaustive()
    }
}
impl fmt::Display for LoanedAllocationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Incident owner capacity exhausted")
    }
}
impl Error for LoanedAllocationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// An actual JSON failure whose original admitted decode capacity remains retained.
/// The cause drops before its loan; getters borrow the genuine parser error.
pub struct LoanedCodecFailure {
    source: serde_json::Error,
    _custody: DiagnosticRecoveryDecodeErrorLoan,
}
impl LoanedCodecFailure {
    /// Borrow the actual parser/I/O cause, including its category and position.
    pub const fn parser_error(&self) -> &serde_json::Error {
        &self.source
    }
}
impl fmt::Debug for LoanedCodecFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoanedCodecFailure").finish_non_exhaustive()
    }
}
impl fmt::Display for LoanedCodecFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invalid incident record encoding")
    }
}
impl Error for LoanedCodecFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}
pub(in crate::incident) fn decode<T>(
    phase: &mut DiagnosticRecoveryDecodeLoan,
    operation: impl FnOnce() -> Result<T, IncidentStoreError>,
) -> Result<T, IncidentStoreError> {
    let custody = phase.retain_error()?;
    operation().map_err(|cause| match cause {
        IncidentStoreError::Codec(source) => {
            IncidentStoreError::RetainedCodec(LoanedCodecFailure { source, _custody: custody })
        }
        other => other,
    })
}
struct LoanedValidationFailure {
    source: DiagnosticRecoveryValidationError,
    _custody: DiagnosticRecoveryDecodeErrorLoan,
}
#[repr(C)]
struct ValidationArcStorage<T> {
    strong: std::sync::atomic::AtomicUsize,
    weak: std::sync::atomic::AtomicUsize,
    payload: T,
}
const fn validation_arc_bytes(payload_bytes: usize, alignment: usize) -> usize {
    let alignment = if alignment > align_of::<usize>() { alignment } else { align_of::<usize>() };
    let bytes = 2 * size_of::<usize>() + payload_bytes;
    bytes.div_ceil(alignment) * alignment
}
const _: () = {
    assert!(size_of::<IncidentStoreError>() <= DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_BYTES);
    assert!(align_of::<IncidentStoreError>() <= DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_ALIGNMENT);
    assert!(size_of::<DiagnosticStorageError>() <= DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_BYTES);
    assert!(
        align_of::<DiagnosticStorageError>() <= DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_ALIGNMENT
    );
    assert!(size_of::<IncidentError>() <= DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_BYTES);
    assert!(align_of::<IncidentError>() <= DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_ALIGNMENT);
    assert!(
        size_of::<DiagnosticRecoveryValidationError>()
            <= DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_BYTES
    );
    assert!(
        align_of::<DiagnosticRecoveryValidationError>()
            <= DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_ALIGNMENT
    );
    assert!(
        size_of::<ValidationArcStorage<IncidentStoreError>>()
            <= validation_arc_bytes(
                DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_BYTES,
                DIAGNOSTIC_INCIDENT_VALIDATION_PAYLOAD_ALIGNMENT
            )
    );
    assert!(size_of::<LoanedValidationFailure>() <= DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_BYTES);
    assert!(
        align_of::<LoanedValidationFailure>() <= DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_ALIGNMENT
    );
    assert!(
        size_of::<ValidationArcStorage<LoanedValidationFailure>>()
            <= validation_arc_bytes(
                DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_BYTES,
                DIAGNOSTIC_INCIDENT_VALIDATION_OUTER_ALIGNMENT
            )
    );
};
impl super::validation_failure::ValidationFailure {
    fn into_error(
        self,
        allocation: DiagnosticIncidentValidationCauseLoan,
    ) -> DiagnosticRecoveryValidationError {
        use super::validation_failure::ValidationSource;
        let (code, source) = self.into_source();
        let source: Arc<dyn Error + Send + Sync> = match source {
            ValidationSource::None => return DiagnosticRecoveryValidationError::rejected(code),
            ValidationSource::Store(source) => Arc::new(source),
            ValidationSource::Storage(source) => Arc::new(source),
            ValidationSource::Facts(source) => Arc::new(source),
            ValidationSource::Existing(source) => Arc::new(source),
        };
        DiagnosticRecoveryValidationError::from_incident_cause(
            code,
            DiagnosticIncidentValidationCause::from_incident_source(source, allocation),
        )
    }
}
impl fmt::Debug for LoanedValidationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoanedValidationFailure").finish_non_exhaustive()
    }
}
impl fmt::Display for LoanedValidationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(f)
    }
}
impl Error for LoanedValidationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}
pub(in crate::incident) fn validation<T>(
    phase: &mut DiagnosticRecoveryDecodeLoan,
    operation: impl FnOnce() -> Result<T, super::validation_failure::ValidationFailure>,
) -> Result<T, DiagnosticRecoveryValidationError> {
    let (inner, outer) = phase
        .incident_validation_controls()
        .map_err(DiagnosticRecoveryValidationError::incident_allocation_rejected)?
        .into_parts();
    let custody = phase
        .retain_error()
        .map_err(DiagnosticRecoveryValidationError::incident_allocation_rejected)?;
    operation().map_err(|failure| {
        let source = failure.into_error(inner);
        let code = source.code();
        let source = Arc::new(LoanedValidationFailure { source, _custody: custody });
        DiagnosticRecoveryValidationError::from_incident_cause(
            code,
            DiagnosticIncidentValidationCause::from_incident_source(source, outer),
        )
    })
}
