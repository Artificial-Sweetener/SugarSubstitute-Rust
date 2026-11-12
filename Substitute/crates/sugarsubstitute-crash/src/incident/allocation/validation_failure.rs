// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact callback leaves inline with categorical codes in closed discriminants.
use super::super::IncidentStoreError;
use super::{LoanedAllocationFailure, LoanedCodecFailure};
use sugarsubstitute_contracts::{incident::IncidentError, lifecycle::IdentityError};
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticRecoveryValidationCode as Code, DiagnosticRecoveryValidationError,
    DiagnosticStorageError,
};

/// Own genuine typed data until one consuming, preadmitted source-control boundary.
/// The largest native sources carry their code in this discriminant, avoiding a second
/// field beside their complete operation/cause data. No cause allocation occurs here.
pub(in crate::incident) enum ValidationFailure {
    Rejected(Code),
    Store { code: Code, source: StoreLeaf },
    StoreSchema(DiagnosticStorageError),
    StoreJournal(DiagnosticStorageError),
    StoreLedger(DiagnosticStorageError),
    StoreArithmetic(DiagnosticStorageError),
    StoreIdentity(DiagnosticStorageError),
    StorageSchema(DiagnosticStorageError),
    StorageJournal(DiagnosticStorageError),
    StorageLedger(DiagnosticStorageError),
    StorageArithmetic(DiagnosticStorageError),
    StorageIdentity(DiagnosticStorageError),
    Facts { code: Code, source: IncidentError },
    Existing(DiagnosticRecoveryValidationError),
}
/// Losslessly retain the smaller incident leaves, separate from their complete native variant.
pub(in crate::incident) enum StoreLeaf {
    MigrationValidation(DiagnosticRecoveryValidationError),
    RecordBound,
    UnsupportedSchema(u32),
    Codec(serde_json::Error),
    RetainedCodec(LoanedCodecFailure),
    Allocation(LoanedAllocationFailure),
    Facts(IncidentError),
    Identity(IdentityError),
    RecoveryConflict,
    Capacity,
    ChangedDuplicate,
    StaleRevision,
    RetainedPayloadRequiresDisposition,
    MissingPayload,
    WrongDigest,
}
/// Transfer original typed causes to the single source-owned Arc construction match.
pub(super) enum ValidationSource {
    None,
    Store(IncidentStoreError),
    Storage(DiagnosticStorageError),
    Facts(IncidentError),
    Existing(DiagnosticRecoveryValidationError),
}
impl ValidationFailure {
    pub(in crate::incident) const fn rejected(code: Code) -> Self {
        Self::Rejected(code)
    }
    pub(in crate::incident) fn store(code: Code, source: IncidentStoreError) -> Self {
        let source = match source {
            IncidentStoreError::Storage(source) => {
                return match code {
                    Code::InvalidSchema => Self::StoreSchema(source),
                    Code::InvalidJournal => Self::StoreJournal(source),
                    Code::LedgerMismatch => Self::StoreLedger(source),
                    Code::ArithmeticOverflow => Self::StoreArithmetic(source),
                    Code::InvalidIdentity => Self::StoreIdentity(source),
                };
            }
            IncidentStoreError::MigrationValidation(source) => {
                StoreLeaf::MigrationValidation(source)
            }
            IncidentStoreError::RecordBound => StoreLeaf::RecordBound,
            IncidentStoreError::UnsupportedSchema(schema) => StoreLeaf::UnsupportedSchema(schema),
            IncidentStoreError::Codec(source) => StoreLeaf::Codec(source),
            IncidentStoreError::RetainedCodec(source) => StoreLeaf::RetainedCodec(source),
            IncidentStoreError::Allocation(source) => StoreLeaf::Allocation(source),
            IncidentStoreError::Facts(source) => StoreLeaf::Facts(source),
            IncidentStoreError::Identity(source) => StoreLeaf::Identity(source),
            IncidentStoreError::RecoveryConflict => StoreLeaf::RecoveryConflict,
            IncidentStoreError::Capacity => StoreLeaf::Capacity,
            IncidentStoreError::ChangedDuplicate => StoreLeaf::ChangedDuplicate,
            IncidentStoreError::StaleRevision => StoreLeaf::StaleRevision,
            IncidentStoreError::RetainedPayloadRequiresDisposition => {
                StoreLeaf::RetainedPayloadRequiresDisposition
            }
            IncidentStoreError::MissingPayload => StoreLeaf::MissingPayload,
            IncidentStoreError::WrongDigest => StoreLeaf::WrongDigest,
        };
        Self::Store { code, source }
    }
    pub(in crate::incident) fn storage(code: Code, source: DiagnosticStorageError) -> Self {
        match code {
            Code::InvalidSchema => Self::StorageSchema(source),
            Code::InvalidJournal => Self::StorageJournal(source),
            Code::LedgerMismatch => Self::StorageLedger(source),
            Code::ArithmeticOverflow => Self::StorageArithmetic(source),
            Code::InvalidIdentity => Self::StorageIdentity(source),
        }
    }
    pub(in crate::incident) fn facts(code: Code, source: IncidentError) -> Self {
        Self::Facts { code, source }
    }
    pub(super) fn into_source(self) -> (Code, ValidationSource) {
        use ValidationSource as Source;
        match self {
            Self::Rejected(code) => (code, Source::None),
            Self::Store { code, source } => (code, Source::Store(source.into_error())),
            Self::StoreSchema(source) => {
                (Code::InvalidSchema, Source::Store(IncidentStoreError::Storage(source)))
            }
            Self::StoreJournal(source) => {
                (Code::InvalidJournal, Source::Store(IncidentStoreError::Storage(source)))
            }
            Self::StoreLedger(source) => {
                (Code::LedgerMismatch, Source::Store(IncidentStoreError::Storage(source)))
            }
            Self::StoreArithmetic(source) => {
                (Code::ArithmeticOverflow, Source::Store(IncidentStoreError::Storage(source)))
            }
            Self::StoreIdentity(source) => {
                (Code::InvalidIdentity, Source::Store(IncidentStoreError::Storage(source)))
            }
            Self::StorageSchema(source) => (Code::InvalidSchema, Source::Storage(source)),
            Self::StorageJournal(source) => (Code::InvalidJournal, Source::Storage(source)),
            Self::StorageLedger(source) => (Code::LedgerMismatch, Source::Storage(source)),
            Self::StorageArithmetic(source) => (Code::ArithmeticOverflow, Source::Storage(source)),
            Self::StorageIdentity(source) => (Code::InvalidIdentity, Source::Storage(source)),
            Self::Facts { code, source } => (code, Source::Facts(source)),
            Self::Existing(source) => (source.code(), Source::Existing(source)),
        }
    }
}
impl StoreLeaf {
    fn into_error(self) -> IncidentStoreError {
        match self {
            Self::MigrationValidation(source) => IncidentStoreError::MigrationValidation(source),
            Self::RecordBound => IncidentStoreError::RecordBound,
            Self::UnsupportedSchema(schema) => IncidentStoreError::UnsupportedSchema(schema),
            Self::Codec(source) => IncidentStoreError::Codec(source),
            Self::RetainedCodec(source) => IncidentStoreError::RetainedCodec(source),
            Self::Allocation(source) => IncidentStoreError::Allocation(source),
            Self::Facts(source) => IncidentStoreError::Facts(source),
            Self::Identity(source) => IncidentStoreError::Identity(source),
            Self::RecoveryConflict => IncidentStoreError::RecoveryConflict,
            Self::Capacity => IncidentStoreError::Capacity,
            Self::ChangedDuplicate => IncidentStoreError::ChangedDuplicate,
            Self::StaleRevision => IncidentStoreError::StaleRevision,
            Self::RetainedPayloadRequiresDisposition => {
                IncidentStoreError::RetainedPayloadRequiresDisposition
            }
            Self::MissingPayload => IncidentStoreError::MissingPayload,
            Self::WrongDigest => IncidentStoreError::WrongDigest,
        }
    }
}
impl From<DiagnosticRecoveryValidationError> for ValidationFailure {
    fn from(source: DiagnosticRecoveryValidationError) -> Self {
        Self::Existing(source)
    }
}
const _: () = {
    assert!(size_of::<ValidationFailure>() <= 128);
    assert!(align_of::<ValidationFailure>() <= 8);
};
