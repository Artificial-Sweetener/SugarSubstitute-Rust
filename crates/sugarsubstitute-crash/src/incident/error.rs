// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve malformed-record causes without exposing persisted bytes or native paths.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{incident::IncidentError, lifecycle::IdentityError};

/// Stable incident-owner failure categories without persisted or native text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncidentStoreFailureCode {
    /// The protected adapter retained an external failure.
    Storage,
    /// Strict record encoding or decoding failed.
    Codec,
    /// The persisted schema is unsupported.
    UnknownSchema,
    /// Related incident or recovery facts are inconsistent.
    Corrupt,
    /// A required committed payload or record was unavailable.
    MissingPayload,
    /// Actual bytes differ from the complete expected digest.
    WrongDigest,
    /// A duplicate identity supplied different immutable content.
    ChangedDuplicate,
    /// The expected revision no longer matches the actual predecessor.
    StaleRevision,
    /// Existing evidence must complete its protected disposition before publication.
    RetainedPayloadRequiresDisposition,
    /// A literal reservation or bounded counter prevented admission.
    Capacity,
    /// Cumulative summary eviction count cannot advance without wrapping.
    EvictionCountExhausted,
    /// The original deadline elapsed or its monotonic clock failed.
    Deadline,
    /// Original operation cancellation was requested.
    Cancelled,
}

/// A bounded incident record could not be validated or committed.
pub enum IncidentStoreError {
    /// Encoded bytes exceeded the format's fixed limit.
    RecordBound,
    /// The format's schema is outside its explicit accepted version set.
    UnsupportedSchema(u32),
    /// Strict JSON decoding or encoding failed; original bytes remain with storage.
    Codec(serde_json::Error),
    /// Native parsing failed while its original decode allocation remains retained.
    #[cfg(windows)]
    RetainedCodec(super::allocation::LoanedCodecFailure),
    /// A real reserve failure retains its original admitted allocation destination.
    #[cfg(windows)]
    Allocation(super::allocation::LoanedAllocationFailure),
    /// A domain constructor rejected internally inconsistent facts.
    Facts(IncidentError),
    /// A scoped lifecycle identity was invalid.
    Identity(IdentityError),
    /// A recovery record disagreed with the retained journal or ledger.
    RecoveryConflict,
    /// A literal retained-owner or transaction bound prevents admission.
    Capacity,
    /// Existing identity matched but immutable committed bytes differed.
    ChangedDuplicate,
    /// Current observed revision differs from the requested predecessor.
    StaleRevision,
    /// Existing retained evidence cannot be replaced before protected disposition settles.
    RetainedPayloadRequiresDisposition,
    /// Required actual payload or committed record was unavailable.
    MissingPayload,
    /// Actual complete bytes disagreed with their expected digest.
    WrongDigest,
    /// Actual protected storage failed; the original categorical source is retained.
    #[cfg(windows)]
    Storage(sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageError),
}
impl IncidentStoreError {
    /// Observe a categorical cause while its original typed source remains retained.
    pub const fn code(&self) -> IncidentStoreFailureCode {
        use IncidentStoreFailureCode as Code;
        match self {
            Self::RecordBound | Self::Capacity => Code::Capacity,
            Self::UnsupportedSchema(_) => Code::UnknownSchema,
            Self::Codec(_) => Code::Codec,
            #[cfg(windows)]
            Self::RetainedCodec(_) => Code::Codec,
            #[cfg(windows)]
            Self::Allocation(_) => Code::Capacity,
            Self::Facts(IncidentError::EvictionCountExhausted) => Code::EvictionCountExhausted,
            Self::Facts(_) | Self::Identity(_) | Self::RecoveryConflict => Code::Corrupt,
            Self::ChangedDuplicate => Code::ChangedDuplicate,
            Self::StaleRevision => Code::StaleRevision,
            Self::RetainedPayloadRequiresDisposition => Code::RetainedPayloadRequiresDisposition,
            Self::MissingPayload => Code::MissingPayload,
            Self::WrongDigest => Code::WrongDigest,
            #[cfg(windows)]
            Self::Storage(cause) => {
                use sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageFailureCode as Storage;
                match cause.code() {
                    Storage::Deadline => Code::Deadline,
                    Storage::Cancelled => Code::Cancelled,
                    Storage::Capacity => Code::Capacity,
                    _ => Code::Storage,
                }
            }
        }
    }
}
impl fmt::Debug for IncidentStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::RecordBound => "RecordBound",
            Self::UnsupportedSchema(_) => "UnsupportedSchema",
            Self::Codec(_) => "Codec",
            #[cfg(windows)]
            Self::RetainedCodec(_) => "Codec",
            Self::Facts(_) => "Facts",
            Self::Identity(_) => "Identity",
            Self::RecoveryConflict => "RecoveryConflict",
            Self::Capacity => "Capacity",
            #[cfg(windows)]
            Self::Allocation(_) => "Capacity",
            Self::ChangedDuplicate => "ChangedDuplicate",
            Self::StaleRevision => "StaleRevision",
            Self::RetainedPayloadRequiresDisposition => "RetainedPayloadRequiresDisposition",
            Self::MissingPayload => "MissingPayload",
            Self::WrongDigest => "WrongDigest",
            #[cfg(windows)]
            Self::Storage(_) => "Storage",
        };
        f.debug_struct("IncidentStoreError")
            .field("owner", &"sugarsubstitute-crash")
            .field("code", &code)
            .finish()
    }
}
impl fmt::Display for IncidentStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordBound => f.write_str("Incident record exceeds its admitted bound"),
            Self::UnsupportedSchema(version) => write!(f, "Unsupported incident schema {version}"),
            Self::Codec(_) => f.write_str("Invalid incident record encoding"),
            #[cfg(windows)]
            Self::RetainedCodec(_) => f.write_str("Invalid incident record encoding"),
            Self::Facts(_) => f.write_str("Invalid incident record facts"),
            Self::Identity(_) => f.write_str("Invalid incident record identity"),
            Self::RecoveryConflict => f.write_str("Incident recovery facts disagree"),
            Self::Capacity => f.write_str("Incident owner capacity exhausted"),
            #[cfg(windows)]
            Self::Allocation(_) => f.write_str("Incident owner capacity exhausted"),
            Self::ChangedDuplicate => f.write_str("Incident duplicate content changed"),
            Self::StaleRevision => f.write_str("Incident expected revision changed"),
            Self::RetainedPayloadRequiresDisposition => f.write_str("Retained incident evidence requires settled protected disposition before publication"),
            Self::MissingPayload => f.write_str("Required incident content unavailable"),
            Self::WrongDigest => f.write_str("Incident content digest disagrees"),
            #[cfg(windows)]
            Self::Storage(_) => f.write_str("Protected incident storage failed"),
        }
    }
}
impl Error for IncidentStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            #[cfg(windows)]
            Self::RetainedCodec(error) => Some(error.parser_error()),
            #[cfg(windows)]
            Self::Allocation(error) => Some(error.reserve_error()),
            Self::Facts(error) => Some(error),
            Self::Identity(error) => Some(error),
            #[cfg(windows)]
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}
#[cfg(windows)]
impl From<sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageError>
    for IncidentStoreError
{
    fn from(
        error: sugarsubstitute_supervision::diagnostic_storage::DiagnosticStorageError,
    ) -> Self {
        Self::Storage(error)
    }
}
impl From<IncidentError> for IncidentStoreError {
    fn from(error: IncidentError) -> Self {
        Self::Facts(error)
    }
}
impl From<IdentityError> for IncidentStoreError {
    fn from(error: IdentityError) -> Self {
        Self::Identity(error)
    }
}
impl From<serde_json::Error> for IncidentStoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Codec(error)
    }
}
