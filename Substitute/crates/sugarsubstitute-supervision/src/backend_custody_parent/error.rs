// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original custody admission and native failure causes.
use super::*;
/// Preserve actual original admission failures before any controller allocation.
pub enum BackendCustodyParentInputError {
    /// Original session absent.
    Missing,
    /// Compiled controller/native producer cannot fit the original partition.
    Layout,
    /// Actual central source allocation rejected.
    Admission(sugarsubstitute_contracts::execution::AdmissionError),
}
impl std::fmt::Debug for BackendCustodyParentInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BackendCustodyParentInputError { redacted }")
    }
}
impl std::fmt::Display for BackendCustodyParentInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Backend custody parent admission rejected")
    }
}
impl std::error::Error for BackendCustodyParentInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(cause) => Some(cause),
            _ => None,
        }
    }
}
/// Retain the actual escaped cause before its same original parent metadata.
#[derive(Clone)]
pub struct BackendCustodyParentFailure {
    pub(super) cause: NativeObservationError,
    pub(super) storage: Arc<BackendCustodyParentStorage>,
}
impl std::fmt::Debug for BackendCustodyParentFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::fmt::Display for BackendCustodyParentFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for BackendCustodyParentFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
impl BackendCustodyParentFailure {
    /// Observe the retained actual source key.
    pub fn key(&self) -> BackendCustodyKey {
        self.storage.key()
    }
}
