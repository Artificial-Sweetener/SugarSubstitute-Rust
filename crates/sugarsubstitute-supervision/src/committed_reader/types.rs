// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define closed objects, same-origin slots and custody-preserving failures.
use super::*;
use sugarsubstitute_contracts::incident::{AttachmentKind, IncidentKey, IncidentRevision};
/// Select only the three records needed for committed incident validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommittedReadObject {
    /// Current namespace ledger, retaining its historical operation identities.
    StoreState,
    /// One incident's current schema2 summary.
    Summary(IncidentKey),
    /// Immutable payload revision decoded from the protected summary.
    Minidump(IncidentKey, IncidentRevision),
}
impl CommittedReadObject {
    pub(super) fn index(self) -> usize {
        match self {
            Self::Summary(_) => 0,
            Self::StoreState => 1,
            Self::Minidump(..) => 2,
        }
    }
    pub(super) fn object(self) -> crate::diagnostic_storage::DiagnosticObject {
        use crate::diagnostic_storage::DiagnosticObject;
        match self {
            Self::StoreState => DiagnosticObject::StoreState,
            Self::Summary(key) => DiagnosticObject::Summary(key),
            Self::Minidump(key, revision) => {
                DiagnosticObject::Payload(key, revision, AttachmentKind::Minidump)
            }
        }
    }
}
/// Borrow only a file actually opened by the same reader; consumers cannot construct it.
pub struct CommittedReadSlot {
    pub(super) object: CommittedReadObject,
    pub(super) original: CommittedReaderOrigin,
}
impl CommittedReadSlot {
    /// Return inert selected object facts without granting a handle or path.
    pub const fn object(&self) -> CommittedReadObject {
        self.object
    }
}
/// Keep genuine original admission after a construction rejection before native effects.
pub struct CommittedReaderConstructionFailure {
    /// Original bounded cause; no failure-time allocation is needed for inline denial.
    pub cause: ClaimError,
    pub(super) original: CommittedReaderOrigin,
}
impl CommittedReaderConstructionFailure {
    /// Borrow the original paired target without exposing its memory claim.
    pub fn original_origin(&self) -> &CommittedReaderOrigin {
        &self.original
    }
}
impl std::fmt::Debug for CommittedReaderConstructionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommittedReaderConstructionFailure")
            .field("cause", &self.cause)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for CommittedReaderConstructionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::error::Error for CommittedReaderConstructionFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
/// Return identical physical custody after any uncertain checked close.
pub struct CommittedReaderRetirementFailure {
    /// Same fence, pins, partial outputs and original charge.
    pub owner: CommittedDiagnosticReader,
    /// Genuine first native or paired cleanup failure.
    pub cause: ClaimError,
}
