// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve semantic decoder causes behind redacted recovery categories.
use super::incident_cause::DiagnosticIncidentValidationCause;
use crate::diagnostic_storage::DiagnosticStorageError;
use std::{error::Error, fmt, sync::Arc};
/// Categorize a bounded semantic decision without native privilege or sensitive text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticRecoveryValidationCode {
    /// A required closed schema is unsupported or malformed.
    InvalidSchema,
    /// Full journal identity, intent or victim facts disagree.
    InvalidJournal,
    /// The actual full ledger is neither the exact original nor candidate.
    LedgerMismatch,
    /// A checked cumulative increment or bound overflowed.
    ArithmeticOverflow,
    /// Bounded operation, namespace, digest or completion identity disagrees.
    InvalidIdentity,
}
/// Retain the exact semantic cause without formatting its potentially sensitive text.
#[derive(Clone)]
pub struct DiagnosticRecoveryValidationError {
    detail: ValidationFailureDetail,
}
#[derive(Clone)]
enum ValidationFailureDetail {
    Rejected(DiagnosticRecoveryValidationCode),
    Ordinary { code: DiagnosticRecoveryValidationCode, cause: Arc<dyn Error + Send + Sync> },
    Incident { code: DiagnosticRecoveryValidationCode, cause: DiagnosticIncidentValidationCause },
    Admission(DiagnosticStorageError),
}
impl DiagnosticRecoveryValidationError {
    /// Reject an inconsistent semantic decision without inventing a native cause.
    pub const fn rejected(code: DiagnosticRecoveryValidationCode) -> Self {
        Self { detail: ValidationFailureDetail::Rejected(code) }
    }
    /// Preserve an actual decoder failure while exposing only its category.
    pub fn caused(
        code: DiagnosticRecoveryValidationCode,
        cause: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self { detail: ValidationFailureDetail::Ordinary { code, cause: Arc::new(cause) } }
    }
    /// Move an opaque already-admitted Incident source without allocating.
    /// The cause retains its fixed outside control loan through its last alias.
    pub fn from_incident_cause(
        code: DiagnosticRecoveryValidationCode,
        cause: DiagnosticIncidentValidationCause,
    ) -> Self {
        Self { detail: ValidationFailureDetail::Incident { code, cause } }
    }
    /// Retain a genuine pre-parse allocation rejection inline without an Arc/Box.
    /// Its original categorical operation and post-control facts remain observable.
    pub fn incident_allocation_rejected(cause: DiagnosticStorageError) -> Self {
        Self { detail: ValidationFailureDetail::Admission(cause) }
    }
    /// Observe the redacted semantic failure category.
    pub const fn code(&self) -> DiagnosticRecoveryValidationCode {
        match &self.detail {
            ValidationFailureDetail::Rejected(code)
            | ValidationFailureDetail::Ordinary { code, .. }
            | ValidationFailureDetail::Incident { code, .. } => *code,
            ValidationFailureDetail::Admission(_) => {
                DiagnosticRecoveryValidationCode::InvalidIdentity
            }
        }
    }
}
impl fmt::Debug for DiagnosticRecoveryValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticRecoveryValidationError")
            .field("code", &self.code())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for DiagnosticRecoveryValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Diagnostic recovery validation failed: {:?}", self.code())
    }
}
impl Error for DiagnosticRecoveryValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.detail {
            ValidationFailureDetail::Rejected(_) => None,
            ValidationFailureDetail::Ordinary { cause, .. } => Some(cause.as_ref()),
            ValidationFailureDetail::Incident { cause, .. } => cause.source(),
            ValidationFailureDetail::Admission(cause) => Some(cause),
        }
    }
}

const _: () = assert!(std::mem::size_of::<DiagnosticRecoveryValidationError>() <= 120);
