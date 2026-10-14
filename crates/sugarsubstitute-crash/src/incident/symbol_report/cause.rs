// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep typed original source, worker and retirement causes without sensitive diagnostics.
use super::source::{DataOwner, Source};
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{AdmissionError, PanicCause, TerminalKind};
use sugarsubstitute_supervision::ClaimError;
/// Genuine first failure or exact inert parser/provider rejection category.
pub enum SymbolReportCause {
    /// Original Core cancellation, deadline, clock or admission failure.
    Admission(AdmissionError),
    /// Original native reader or installed-release failure.
    Native(ClaimError),
    /// Actual decode destination rejection before any read.
    Decode(super::super::DumpReaderConstructionCause),
    /// Explicit native UTC adapter failure.
    Utc(super::super::IncidentUtcReadError),
    /// Actual strict committed dump rejection; original source stays in its owner.
    Dump(super::super::DumpReadCode),
    /// Actual same-file exception/module rejection.
    Frame(super::super::SymbolFrameError),
    /// Actual provider rejection; original source stays in its owner.
    Provider(super::super::SymbolProviderFailureCode),
    /// Genuine execution worker panic.
    Panic(PanicCause),
    /// Genuine execution cancellation or terminal failure.
    Stopped(TerminalKind),
    /// A required retained owner was unavailable.
    State,
}
impl fmt::Debug for SymbolReportCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SymbolReportCause")
    }
}
impl fmt::Display for SymbolReportCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Symbol report failed")
    }
}
impl Error for SymbolReportCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Native(e) => Some(e),
            Self::Utc(e) => Some(e),
            Self::Decode(super::super::DumpReaderConstructionCause::Control(e)) => Some(e),
            Self::Decode(super::super::DumpReaderConstructionCause::Allocation(e)) => Some(e),
            _ => None,
        }
    }
}
/// Own every original failed capsule and its charge until actual final allocation Drop.
pub struct SymbolReportFailure {
    pub(super) causes: [Option<SymbolReportCause>; 4],
    pub(super) data: Option<DataOwner>,
    pub(super) settled: bool,
    pub(super) _source: Source,
}
impl SymbolReportFailure {
    /// Borrow original copied categories and worker failures without taking custody.
    pub fn causes(&self) -> impl Iterator<Item = &SymbolReportCause> {
        self.causes.iter().flatten().chain(
            self.data.iter().flat_map(|data| data.first.iter().chain(data.retirement.iter())),
        )
    }
    /// True only after actual ordered checked cleanup, independent of failed Main outcome.
    pub fn settled(&self) -> bool {
        self.settled
    }
}
impl fmt::Debug for SymbolReportFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SymbolReportFailure")
    }
}
impl fmt::Display for SymbolReportFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Symbol report failed")
    }
}
impl Error for SymbolReportFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        let data = self.data.as_ref();
        if let Some(owner) =
            data.and_then(|data| data.reader.as_ref()).filter(|owner| owner.source().is_some())
        {
            return Some(owner);
        }
        if let Some(owner) =
            data.and_then(|data| data.dump.as_ref()).filter(|owner| owner.source().is_some())
        {
            return Some(owner);
        }
        if let Some(owner) =
            data.and_then(|data| data.provider.as_ref()).filter(|owner| owner.source().is_some())
        {
            return Some(owner);
        }
        self.causes().next().map(|cause| cause as &(dyn Error + 'static))
    }
}
