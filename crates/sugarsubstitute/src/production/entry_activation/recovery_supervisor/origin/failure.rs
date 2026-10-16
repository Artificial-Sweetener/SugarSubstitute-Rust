// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep original publication and independent cleanup causes in separate typed categories.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum OriginFailureCategory {
    Memory,
    Grant,
    Publication,
    RepositoryCleanup,
}
/// Actual provider error stays charged with the retained operation and its original guard.
#[derive(Debug)]
pub(super) struct FailureData {
    category: OriginFailureCategory,
    cause: Cause,
}
#[derive(Debug)]
enum Cause {
    Memory(MemoryClaimError),
    Grant(AdmissionError),
    Store(IncidentStoreError),
}
impl FailureData {
    pub(super) fn memory(cause: MemoryClaimError) -> Self {
        Self { category: OriginFailureCategory::Memory, cause: Cause::Memory(cause) }
    }
    pub(super) fn grant(cause: AdmissionError) -> Self {
        Self { category: OriginFailureCategory::Grant, cause: Cause::Grant(cause) }
    }
    pub(super) fn store(category: OriginFailureCategory, cause: IncidentStoreError) -> Self {
        Self { category, cause: Cause::Store(cause) }
    }
    /// Return finite policy category while preserving the complete unchanged cause.
    pub(super) const fn category(&self) -> OriginFailureCategory {
        self.category
    }
}
impl std::fmt::Display for FailureData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recovery origin failed: {:?}", self.category)
    }
}
impl Error for FailureData {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match &self.cause {
            Cause::Memory(cause) => cause,
            Cause::Grant(cause) => cause,
            Cause::Store(cause) => cause,
        })
    }
}

/// Escaped causes keep the same source and outside projection claim until their last drop.
/// The cause allocation is released before its source guard; no provider runs under the slot lock.
pub(in crate::production) struct OriginFailure {
    pub(super) cause: Arc<FailureData>,
    pub(super) _source: super::source::Source,
}
impl OriginFailure {
    /// Return the finite original or checked-cleanup category, preserving all native causes.
    pub(in crate::production) fn category(&self) -> OriginFailureCategory {
        self.cause.category()
    }
}
impl std::fmt::Debug for OriginFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OriginFailure").field("category", &self.category()).finish_non_exhaustive()
    }
}
impl std::fmt::Display for OriginFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.cause, f)
    }
}
impl Error for OriginFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
