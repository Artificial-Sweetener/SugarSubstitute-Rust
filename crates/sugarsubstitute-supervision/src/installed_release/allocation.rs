// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Partition the sole admitted capture claim into closed retained-owner regions.

mod loans;
pub(super) mod state;

pub(crate) use loans::{CaptureLoan, SharedCaptureLoan};
pub(crate) use state::AllocationOrigin;

use std::{error::Error, fmt};

pub(super) const PATH_BYTES: usize = 192 * 1024;
pub(super) const PATH_SLOTS: usize = 8;
pub(super) const SCRATCH_BYTES: usize = 256 * 1024;
pub(super) const METADATA_BYTES: usize = 64 * 1024;
pub(super) const INGRESS_BYTES: usize = 192 * 1024;
pub(super) const CAUSE_SLOTS: usize = 200;

/// Reject construction before another allocation or native effect is accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureCapacityFailure {
    pub(super) kind: CapacityFailureKind,
    pub(crate) storage_code: crate::diagnostic_storage::DiagnosticStorageFailureCode,
    pub(crate) storage_after: Option<crate::diagnostic_storage::DiagnosticStorageFailureCode>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapacityFailureKind {
    Overflow,
    Layout,
    Occupied,
    Closed,
    Identity,
    Missing,
}

impl CaptureCapacityFailure {
    pub(crate) const fn new(kind: CapacityFailureKind) -> Self {
        Self {
            kind,
            storage_code: crate::diagnostic_storage::DiagnosticStorageFailureCode::Admission,
            storage_after: None,
        }
    }
}

impl CaptureCapacityFailure {
    pub(crate) const fn identity() -> Self {
        Self::new(CapacityFailureKind::Identity)
    }
    pub(crate) fn same_native_identity(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
    pub(crate) const fn missing() -> Self {
        Self::new(CapacityFailureKind::Missing)
    }
}
impl fmt::Display for CaptureCapacityFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.kind {
            CapacityFailureKind::Overflow => "layout overflow",
            CapacityFailureKind::Layout => "closed storage limit",
            CapacityFailureKind::Occupied => "occupied storage region",
            CapacityFailureKind::Closed => "closed allocation",
            CapacityFailureKind::Identity => "original allocation mismatch",
            CapacityFailureKind::Missing => "missing owner storage",
        };
        write!(f, "Capture release capacity rejected: {reason}")
    }
}
impl Error for CaptureCapacityFailure {}

/// Select one fixed source-owned region; numerical capacities are never public input.
#[derive(Clone, Copy)]
pub(super) enum CaptureRegion {
    Path(u8),
    Scratch,
    Metadata,
    Ingress,
    Cause(u8),
}

/// Borrow one fixed exclusive producing region; it grants no native effect.
pub(crate) struct CaptureScratch {
    allocation: SharedCaptureLoan,
    original: AllocationOrigin,
}
impl CaptureScratch {
    pub(crate) fn original(&self) -> &AllocationOrigin {
        &self.original
    }
    pub(crate) fn allocation(&self) -> SharedCaptureLoan {
        self.allocation.clone()
    }
}
impl AllocationOrigin {
    pub(super) fn scratch(&self) -> Result<CaptureScratch, CaptureCapacityFailure> {
        let allocation =
            SharedCaptureLoan::new(self.reserve(CaptureRegion::Scratch, SCRATCH_BYTES)?)?;
        Ok(CaptureScratch { allocation, original: self.clone() })
    }
}
