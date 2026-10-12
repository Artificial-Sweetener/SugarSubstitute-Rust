// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve malformed-record causes without exposing persisted bytes or native paths.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{incident::IncidentError, lifecycle::IdentityError};

/// A bounded incident record could not be validated or committed.
pub enum IncidentStoreError {
    /// Encoded bytes exceeded the format's fixed limit.
    RecordBound,
    /// The format's schema is outside its explicit accepted version set.
    UnsupportedSchema(u32),
    /// Strict JSON decoding or encoding failed; original bytes remain with storage.
    Codec(serde_json::Error),
    /// A domain constructor rejected internally inconsistent facts.
    Facts(IncidentError),
    /// A scoped lifecycle identity was invalid.
    Identity(IdentityError),
    /// A recovery record disagreed with the retained journal or ledger.
    RecoveryConflict,
}
impl fmt::Debug for IncidentStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::RecordBound => "RecordBound",
            Self::UnsupportedSchema(_) => "UnsupportedSchema",
            Self::Codec(_) => "Codec",
            Self::Facts(_) => "Facts",
            Self::Identity(_) => "Identity",
            Self::RecoveryConflict => "RecoveryConflict",
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
            Self::Facts(_) => f.write_str("Invalid incident record facts"),
            Self::Identity(_) => f.write_str("Invalid incident record identity"),
            Self::RecoveryConflict => f.write_str("Incident recovery facts disagree"),
        }
    }
}
impl Error for IncidentStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Codec(error) => Some(error),
            Self::Facts(error) => Some(error),
            Self::Identity(error) => Some(error),
            _ => None,
        }
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
