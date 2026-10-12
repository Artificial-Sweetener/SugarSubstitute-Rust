// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain complete scoped subjects and bounded matching identities, never capabilities.
use super::IncidentError;
use crate::lifecycle::{CoordinatorKey, RunKey};
use std::num::NonZeroU64;

/// Identify the complete lifetime that supplied diagnostic evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IncidentSubject {
    /// Exact desktop owner, instance and run generation.
    Desktop(RunKey),
    /// Exact supervisor lifetime, separate from every desktop run.
    Supervisor(CoordinatorKey),
}
impl IncidentSubject {
    /// Return the matching coordinator; this does not prove account or path trust.
    pub const fn coordinator(self) -> CoordinatorKey {
        match self {
            Self::Desktop(run) => run.owner,
            Self::Supervisor(owner) => owner,
        }
    }
}
/// Stable one-incident-per-subject identity, independent of publication attempts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IncidentKey(IncidentSubject);
impl IncidentKey {
    /// Retain all scoped identity fields without inventing freshness/authentication.
    pub const fn new(subject: IncidentSubject) -> Self {
        Self(subject)
    }
    /// Return the complete immutable subject.
    pub const fn subject(self) -> IncidentSubject {
        self.0
    }
}
/// Positive summary/seed revision; comparisons do not authorize replacement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IncidentRevision(NonZeroU64);
impl IncidentRevision {
    /// Reject zero before retaining a revision.
    /// # Errors
    /// Returns `InvalidIdentity` for zero.
    pub const fn new(value: u64) -> Result<Self, IncidentError> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(IncidentError::InvalidIdentity),
        }
    }
    /// Return the positive revision number.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
    /// Advance exactly once without wrapping or changing the original.
    /// # Errors
    /// Returns `RevisionExhausted` at `u64::MAX`.
    pub fn next(self) -> Result<Self, IncidentError> {
        Self::new(self.get().checked_add(1).ok_or(IncidentError::RevisionExhausted)?)
    }
}
/// Exact build annotation with at most 64 ASCII letters, digits, underscores/hyphens.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BuildIdentity(Box<str>);
impl BuildIdentity {
    /// Copy validated bounded text; no native path or release trust is inferred.
    /// # Errors
    /// Returns `InvalidText` for empty, oversized or non-annotation text.
    pub fn new(value: &str) -> Result<Self, IncidentError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        {
            return Err(IncidentError::InvalidText);
        }
        Ok(Self(value.into()))
    }
    /// Borrow exact spelling; retained allocation is bounded by validated byte length.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
/// Opaque nonzero finished-report UUID; this value grants no database access.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DumpId([u8; 16]);
impl DumpId {
    /// Retain exact canonical UUID bytes without accepting the absent all-zero value.
    /// # Errors
    /// Returns `InvalidIdentity` for an all-zero UUID.
    pub fn new(bytes: [u8; 16]) -> Result<Self, IncidentError> {
        if bytes == [0; 16] {
            return Err(IncidentError::InvalidIdentity);
        }
        Ok(Self(bytes))
    }
    /// Borrow matching bytes; the capture adapter establishes provenance separately.
    pub const fn bytes(&self) -> &[u8; 16] {
        &self.0
    }
}
