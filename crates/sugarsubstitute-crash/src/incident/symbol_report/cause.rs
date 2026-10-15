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
        match self {
            Self::Dump(code) => write!(f, "Committed dump rejected: {code:?}"),
            Self::Frame(code) => write!(f, "Captured frame rejected: {code:?}"),
            Self::Provider(code) => write!(f, "Symbol provider rejected: {code:?}"),
            Self::State => f.write_str("Symbol report required owner unavailable"),
            Self::Admission(_) => f.write_str("Symbol report admission failed"),
            Self::Native(_) => f.write_str("Symbol report native selection or reader failed"),
            Self::Decode(_) => f.write_str("Symbol report decode admission failed"),
            Self::Utc(_) => f.write_str("Symbol report UTC observation failed"),
            Self::Panic(_) => f.write_str("Symbol report worker panicked"),
            Self::Stopped(kind) => write!(f, "Symbol report worker stopped: {kind:?}"),
        }
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
        self.data
            .iter()
            .flat_map(|data| data.first.iter())
            .chain(self.causes.iter().flatten())
            .chain(self.data.iter().flat_map(|data| data.retirement.iter()))
    }
    /// Borrow primary, post-read and retirement causes in their distinct retained cells.
    /// Healthy committed ownership contributes no error to any of these observations.
    pub fn dump_failures(&self) -> [Option<crate::incident::DumpReadFailure<'_>>; 3] {
        let Some(data) = self.data.as_ref() else { return [None, None, None] };
        if let Some(reader) = data.reader.as_ref() {
            return [reader.failure(), reader.secondary_failure(), reader.retirement_failure()];
        }
        data.dump.as_ref().map_or([None, None, None], |dump| {
            [dump.failure(), dump.secondary_failure(), dump.retirement_failure()]
        })
    }
    /// Borrow provider lookup, post-call and retirement failures without replacing the primary.
    pub fn provider_failures(&self) -> [Option<crate::incident::SymbolProviderFailure<'_>>; 3] {
        self.data.as_ref().and_then(|data| data.provider.as_ref()).map_or(
            [None, None, None],
            |provider| {
                [provider.failure(), provider.secondary_failure(), provider.retirement_failure()]
            },
        )
    }
    /// Follow only the owner named by the original typed operation failure.
    /// Independent later failures remain in their own cells rather than taking primary precedence.
    fn original_source<'a>(&'a self, cause: &'a SymbolReportCause) -> &'a (dyn Error + 'static) {
        if let Some(data) = self.data.as_ref() {
            let retiring = data.retirement.as_ref().is_some_and(|held| std::ptr::eq(held, cause));
            match cause {
                SymbolReportCause::Dump(_)
                | SymbolReportCause::Frame(crate::incident::SymbolFrameError::Source(_)) => {
                    if let Some(reader) = data.reader.as_ref().filter(|reader| {
                        reader.failure().is_some()
                            || (retiring && reader.retirement_failure().is_some())
                    }) {
                        return reader;
                    }
                    if let Some(dump) = data.dump.as_ref().filter(|dump| {
                        dump.failure().is_some()
                            || (retiring && dump.retirement_failure().is_some())
                    }) {
                        return dump;
                    }
                }
                SymbolReportCause::Provider(_) => {
                    if let Some(provider) = data.provider.as_ref().filter(|provider| {
                        provider.construction_failure().is_some()
                            || provider.failure().is_some()
                            || (retiring && provider.retirement_failure().is_some())
                    }) {
                        return provider;
                    }
                }
                _ => {}
            }
        }
        cause
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
        self.causes().next().map(|cause| self.original_source(cause))
    }
}
