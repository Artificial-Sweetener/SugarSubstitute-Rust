// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind protected objects and disposition plans without granting native authority.
use crate::ClaimError;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    incident::{AttachmentKind, IncidentKey, IncidentRevision, LogicalStoreOperationKey},
    lifecycle::NamespaceId,
};

/// Required retained allocation claim for the complete protected storage owner.
pub const DIAGNOSTIC_STORAGE_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum actual live native handle slots, including partial outputs.
pub const DIAGNOSTIC_HANDLE_BOUND: usize = 260;
/// Maximum bytes transferred by any one streaming read or write.
pub const DIAGNOSTIC_IO_BYTES: usize = 64 * 1024;
/// Maximum physical entries observed across one complete store scan.
pub const DIAGNOSTIC_SCAN_BOUND: usize = 4096;
/// Maximum retained seed files in a complete inventory.
pub const DIAGNOSTIC_SEED_BOUND: usize = 128;
/// Maximum simultaneously retained publication stages.
pub const DIAGNOSTIC_STAGE_BOUND: usize = 8;
/// Maximum simultaneously retained read or export leases.
pub const DIAGNOSTIC_READ_BOUND: usize = 4;
/// Maximum original normal operation duration, measured in monotonic nanoseconds.
pub const DIAGNOSTIC_OPERATION_NANOS: u64 = 60_000_000_000;
/// Maximum original checked-cleanup duration, measured in monotonic nanoseconds.
pub const DIAGNOSTIC_CLEANUP_NANOS: u64 = 15_000_000_000;

/// Name only typed protected objects; strings and paths cannot authorize access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticObject {
    /// The complete failed subject's bounded seed.
    Seed(IncidentKey),
    /// The complete failed subject's current immutable summary.
    Summary(IncidentKey),
    /// Current namespace ledger and its closed journal-schema variants.
    StoreState,
    /// One revision's allowlisted payload attachment.
    Payload(IncidentKey, IncidentRevision, AttachmentKind),
    /// One operation's journal, including its original full coordinator key.
    Journal(LogicalStoreOperationKey),
}
impl DiagnosticObject {
    pub(crate) fn namespace(self) -> Option<NamespaceId> {
        match self {
            Self::Seed(key) | Self::Summary(key) | Self::Payload(key, ..) => {
                Some(key.subject().coordinator().namespace)
            }
            Self::Journal(key) => Some(key.owner().namespace),
            Self::StoreState => None,
        }
    }
    pub(crate) fn bound(self) -> u64 {
        match self {
            Self::Seed(_) => 4096,
            Self::Summary(_) | Self::StoreState | Self::Journal(_) => 65536,
            Self::Payload(_, _, AttachmentKind::Minidump) => {
                sugarsubstitute_contracts::incident::MAX_PAYLOAD_BYTES
            }
            Self::Payload(..) => sugarsubstitute_contracts::incident::MAX_TEXT_ATTACHMENT_BYTES,
        }
    }
}

/// Require absence or exact observed predecessor before protected activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticPredecessor {
    /// An existing object cannot be replaced.
    Absent,
    /// Match complete native file identity and actual content digest.
    Exact {
        /// Actual volume/file identity observed under retained native custody.
        file_identity: [u8; 24],
        /// Actual complete file-content SHA-256.
        sha256: [u8; 32],
    },
}

/// Retain exact actual object observations; only the native adapter constructs them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticReadFacts {
    pub(crate) identity: [u8; 24],
    pub(crate) sha256: [u8; 32],
    pub(crate) bytes: u64,
}
impl DiagnosticReadFacts {
    /// Observe the complete actual retained file identity.
    pub const fn file_identity(self) -> [u8; 24] {
        self.identity
    }
    /// Observe the digest validated from actual source bytes.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
    /// Observe actual bytes, independent of staging reservations.
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

/// Categorize a protected boundary failure without sensitive path or native text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticStorageFailureCode {
    /// Original memory/context admission was invalid.
    Admission,
    /// Actual owner retirement has completed or admission was closed.
    Closed,
    /// A live predecessor/exclusion/read lease prevents this effect.
    Busy,
    /// Exact namespace, operation, digest or native identity disagrees.
    InvalidIdentity,
    /// Actual account differs from the authorized protected owner.
    WrongAccount,
    /// A reparse boundary prevents verified ownership.
    Reparse,
    /// Actual security descriptor has foreign or uncertain protection.
    ForeignProtection,
    /// A retained object's actual identity or bytes changed.
    ChangedIdentity,
    /// Bounded enumeration cannot prove the complete inventory.
    InventoryIncomplete,
    /// A literal handle/stage/lease/byte bound prevents admission.
    Capacity,
    /// The original monotonic deadline elapsed or its clock regressed.
    Deadline,
    /// Cancellation is requested; actual owners remain retained.
    Cancelled,
    /// An actual bounded read failed.
    Read,
    /// An actual bounded write failed.
    Write,
    /// Checked native flush/readback failed.
    Flush,
    /// Activation occurred or may have occurred without complete retirement proof.
    ActivationUncertain,
    /// Disposition occurred or may have occurred without actual absence/close proof.
    DeleteUncertain,
    /// Checked close/unlock could not establish owner retirement.
    Retirement,
}

/// Preserve native causes behind categorical, redacted diagnostic formatting.
#[derive(Clone)]
pub struct DiagnosticStorageError {
    operation: Option<LogicalStoreOperationKey>,
    code: DiagnosticStorageFailureCode,
    cause: Option<ClaimError>,
}
impl DiagnosticStorageError {
    pub(super) const fn rejected(
        operation: Option<LogicalStoreOperationKey>,
        code: DiagnosticStorageFailureCode,
    ) -> Self {
        Self { operation, code, cause: None }
    }
    pub(super) fn native(
        operation: Option<LogicalStoreOperationKey>,
        code: DiagnosticStorageFailureCode,
        cause: ClaimError,
    ) -> Self {
        Self { operation, code, cause: Some(cause) }
    }
    /// Observe exact logical operation without exposing filenames.
    pub const fn operation(&self) -> Option<LogicalStoreOperationKey> {
        self.operation
    }
    /// Observe the actionable failure category.
    pub const fn code(&self) -> DiagnosticStorageFailureCode {
        self.code
    }
}
impl fmt::Debug for DiagnosticStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiagnosticStorageError")
            .field("operation", &self.operation)
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for DiagnosticStorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Protected diagnostic storage failed: {:?}", self.code)
    }
}
impl Error for DiagnosticStorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_ref().map(|cause| cause as &dyn Error)
    }
}
