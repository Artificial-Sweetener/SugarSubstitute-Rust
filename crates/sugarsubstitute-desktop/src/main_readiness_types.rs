// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound native document identities independently of presentation or startup policy.

use crate::host_contract::MainIdentity;
use std::{error::Error, fmt};

/// Own a bounded native CDP frame/loader identifier without per-event allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DocumentId {
    bytes: [u8; 128],
    length: u8,
}
impl DocumentId {
    /// Reject malformed identifiers before copying into fixed owned storage.
    pub(crate) fn new(value: &str) -> Result<Self, ReadinessFault> {
        if value.is_empty() || value.len() > 128 || value.contains('\0') {
            return Err(ReadinessFault::MalformedObservation);
        }
        let mut bytes = [0; 128];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(Self { bytes, length: value.len() as u8 })
    }
    /// Compare with a borrowed native label without decoding or allocating a copy.
    pub(crate) fn matches(&self, value: &str) -> bool {
        &self.bytes[..usize::from(self.length)] == value.as_bytes()
    }
}

/// Bind one native evaluation and its completion barrier to the exact document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProbeTarget {
    pub(crate) identity: MainIdentity,
    pub(crate) operation: u64,
    pub(crate) context: i64,
    pub(crate) frame: DocumentId,
    pub(crate) loader: DocumentId,
}

/// Preserve a bounded observer failure independently of potentially sensitive data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReadinessFault {
    MalformedObservation,
    ObservationOverflow,
    WrongOrigin,
    ContextLost,
    NativeNavigationFailed,
    SetupFailed,
    DisabledExtension,
    ForeignReceipt,
    VersionMismatch,
    StaleProbe,
    ProbeBusy,
    MissingDomResult,
    IdentityExhausted,
    Cancelled,
}
impl fmt::Display for ReadinessFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Native frontend readiness rejected: {self:?}")
    }
}
impl Error for ReadinessFault {}

/// Bind only the actual primitive console argument; remote objects are not evaluated.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ConsoleArgument<'a> {
    PrimitiveString(&'a str),
    Other,
}

/// Use the actual console kind when classifying the two installed-source diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConsoleKind {
    Error,
    Warning,
    Other,
}
