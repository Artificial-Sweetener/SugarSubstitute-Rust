// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move original errors to caller custody only after checked closure, retaining their Core charge.
use super::*;
/// Exact original cause, inseparable from the real original Core completion lease.
/// No replacement cause or storage capability is constructed during transfer.
pub(in crate::production) struct OwnedReportingFailure {
    cause: ReportingCause,
    _original: MemoryLease,
}
impl std::fmt::Display for OwnedReportingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.cause.fmt(f)
    }
}
impl std::fmt::Debug for OwnedReportingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedReportingFailure").field("cause", &self.cause).finish_non_exhaustive()
    }
}
impl std::error::Error for OwnedReportingFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
impl ReportingOperation {
    /// Preserve the same source and immutable unsuccessful fence after moving its first cause.
    pub(super) fn take_failure(&mut self, original: MemoryLease) -> Option<OwnedReportingFailure> {
        if !self.retired || self.failure_transferred {
            return None;
        }
        let cause =
            self.first.take().or_else(|| self.retirement.take().map(ReportingCause::Store))?;
        self.failure_transferred = true;
        Some(OwnedReportingFailure { cause, _original: original })
    }
}
