// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve semantic decoder causes behind redacted recovery categories.
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
    code: DiagnosticRecoveryValidationCode,
    cause: Option<Arc<dyn Error + Send + Sync>>,
}
impl DiagnosticRecoveryValidationError {
    /// Reject an inconsistent semantic decision without inventing a native cause.
    pub const fn rejected(code: DiagnosticRecoveryValidationCode) -> Self {
        Self { code, cause: None }
    }
    /// Preserve an actual decoder failure while exposing only its category.
    pub fn caused(
        code: DiagnosticRecoveryValidationCode,
        cause: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self { code, cause: Some(Arc::new(cause)) }
    }
    /// Observe the redacted semantic failure category.
    pub const fn code(&self) -> DiagnosticRecoveryValidationCode {
        self.code
    }
}
impl fmt::Debug for DiagnosticRecoveryValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticRecoveryValidationError")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for DiagnosticRecoveryValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Diagnostic recovery validation failed: {:?}", self.code)
    }
}
impl Error for DiagnosticRecoveryValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_ref().map(|cause| cause.as_ref() as &(dyn Error + 'static))
    }
}
