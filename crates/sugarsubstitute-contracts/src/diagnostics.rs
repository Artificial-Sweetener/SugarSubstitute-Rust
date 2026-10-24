// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Runtime-independent diagnostic correlation grammar; identities convey no native authority.
use std::{error::Error, fmt, num::NonZeroU64};
include!(concat!(env!("OUT_DIR"), "/diagnostics_contract.rs"));
#[cfg(test)]
mod schema;

/// Select the authoritative concern represented by a structured identity field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorrelationKind {
    /// Authored document identity, when the operation has a document owner.
    Document,
    /// ComfyUI workflow identity, when supplied by the workflow owner.
    Workflow,
    /// One native or provisioning request identity.
    Request,
    /// One admitted execution-owner lifetime.
    Execution,
    /// One transport connection lifetime.
    Connection,
}
impl CorrelationKind {
    /// Recognize the canonical host field names without interpreting arbitrary payload fields.
    pub fn from_field(field: &str) -> Option<Self> {
        match field {
            "document_id" => Some(Self::Document),
            "workflow_id" => Some(Self::Workflow),
            "request_id" => Some(Self::Request),
            "execution_id" => Some(Self::Execution),
            "connection_id" => Some(Self::Connection),
            _ => None,
        }
    }
}
/// Retain a validated identity's semantic kind and borrowed, bounded representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Correlation<'a> {
    kind: CorrelationKind,
    value: Identity<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Identity<'a> {
    Sequence(NonZeroU64),
    Label(&'a str),
}
impl<'a> Correlation<'a> {
    /// Bind a positive owner sequence to its explicit concern.
    /// # Errors
    /// Zero represents absence and cannot claim an actual correlated lifetime.
    pub fn sequence(kind: CorrelationKind, value: u64) -> Result<Self, DiagnosticIdentityError> {
        Ok(Self {
            kind,
            value: Identity::Sequence(NonZeroU64::new(value).ok_or(DiagnosticIdentityError)?),
        })
    }
    /// Bind a bounded ASCII identifier without accepting paths, whitespace or diagnostic text.
    /// The supported provisioning protocol permits letters, digits, underscore and hyphen.
    /// # Errors
    /// Rejects empty, excessive or non-identifier text. Validation grants no producer trust.
    pub fn label(kind: CorrelationKind, value: &'a str) -> Result<Self, DiagnosticIdentityError> {
        if value.is_empty()
            || value.len() > DIAGNOSTIC_IDENTITY_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(DiagnosticIdentityError);
        }
        Ok(Self { kind, value: Identity::Label(value) })
    }
    /// Observe the identity's concern, independently of its presentation representation.
    pub const fn kind(self) -> CorrelationKind {
        self.kind
    }
}
impl fmt::Display for Correlation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Identity::Sequence(value) => value.fmt(f),
            Identity::Label(value) => value.fmt(f),
        }
    }
}
/// Borrow one bounded operation label rather than accepting arbitrary diagnostic text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticOperation<'a>(&'a str);
impl<'a> DiagnosticOperation<'a> {
    /// Validate a stable operation code; free-form error text remains with its feature owner.
    /// # Errors
    /// Rejects empty, overlong, non-ASCII and punctuation-bearing operation names.
    pub fn new(value: &'a str) -> Result<Self, DiagnosticIdentityError> {
        if value.is_empty()
            || value.len() > DIAGNOSTIC_OPERATION_BYTES
            || !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(DiagnosticIdentityError);
        }
        Ok(Self(value))
    }
    /// Borrow the validated label without allocating or changing its spelling.
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}
/// Reject invalid diagnostic identity grammar without retaining sensitive supplied text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticIdentityError;
impl fmt::Display for DiagnosticIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invalid diagnostic identity")
    }
}
impl Error for DiagnosticIdentityError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_supported_concern_preserves_kind_and_bounded_identity()
    -> Result<(), DiagnosticIdentityError> {
        for (field, kind) in [
            ("document_id", CorrelationKind::Document),
            ("workflow_id", CorrelationKind::Workflow),
            ("request_id", CorrelationKind::Request),
            ("execution_id", CorrelationKind::Execution),
            ("connection_id", CorrelationKind::Connection),
        ] {
            assert_eq!(CorrelationKind::from_field(field), Some(kind));
            let identity = Correlation::sequence(kind, 42)?;
            assert_eq!(identity.kind(), kind);
            assert_eq!(identity.to_string(), "42");
            assert!(Correlation::sequence(kind, 0).is_err());
            assert_eq!(
                Correlation::label(kind, "request_42-alpha")?.to_string(),
                "request_42-alpha"
            );
            for invalid in ["", "C:\\private", "Bearer token", "https://host", "糖"] {
                assert!(Correlation::label(kind, invalid).is_err());
            }
        }
        assert!(CorrelationKind::from_field("payload").is_none());
        assert!(Correlation::label(CorrelationKind::Request, &"x".repeat(81)).is_err());
        Ok(())
    }
    #[test]
    fn operation_labels_reject_free_form_text_and_preserve_canonical_spelling()
    -> Result<(), DiagnosticIdentityError> {
        assert_eq!(
            DiagnosticOperation::new("startup_readiness_acknowledged")?.as_str(),
            "startup_readiness_acknowledged"
        );
        for invalid in ["", "secret=value", "operation name", "C:\\private", "糖"] {
            assert!(DiagnosticOperation::new(invalid).is_err());
        }
        assert!(DiagnosticOperation::new(&"x".repeat(129)).is_err());
        Ok(())
    }
}
