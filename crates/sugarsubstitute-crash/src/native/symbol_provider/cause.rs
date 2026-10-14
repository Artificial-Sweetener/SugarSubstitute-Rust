// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve categorical native lookup failures with their genuine typed sources.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{execution::AdmissionError, incident::IncidentError};
use sugarsubstitute_supervision::ClaimError;
pub(super) enum ProviderCause {
    Control(AdmissionError),
    Native(windows::core::Error),
    Selection(ClaimError),
    Facts(IncidentError),
    Utf16(std::string::FromUtf16Error),
    Reserve(std::collections::TryReserveError),
    Capacity,
    Busy,
    Identity,
}
/// Borrow one actual provider failure without logging filenames, PDB text or native paths.
pub struct SymbolProviderFailure<'a>(pub(super) &'a ProviderCause);
impl fmt::Debug for SymbolProviderFailure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SymbolProviderFailure")
    }
}
impl fmt::Display for SymbolProviderFailure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Symbol provider operation failed")
    }
}
impl Error for SymbolProviderFailure<'_> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.0 {
            ProviderCause::Control(e) => Some(e),
            ProviderCause::Native(e) => Some(e),
            ProviderCause::Selection(e) => Some(e),
            ProviderCause::Facts(e) => Some(e),
            ProviderCause::Utf16(e) => Some(e),
            ProviderCause::Reserve(e) => Some(e),
            _ => None,
        }
    }
}

/// Copied categorical lookup outcome; actual causes remain in the provider owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolProviderFailureCode {
    /// Genuine original Core controls rejected the operation.
    Control,
    /// Actual native API or export resolution failed.
    Native,
    /// Selected release association or captured module did not match.
    Identity,
    /// Actual fallible buffer allocation failed; original cause remains in the owner.
    Reserve,
    /// A returned allocation capacity exceeded its fixed destination.
    Capacity,
    /// Another actual provider session still holds the process fence.
    Busy,
}
impl ProviderCause {
    pub(super) fn code(&self) -> SymbolProviderFailureCode {
        match self {
            Self::Control(_) => SymbolProviderFailureCode::Control,
            Self::Native(_) => SymbolProviderFailureCode::Native,
            Self::Reserve(_) => SymbolProviderFailureCode::Reserve,
            Self::Capacity => SymbolProviderFailureCode::Capacity,
            Self::Busy => SymbolProviderFailureCode::Busy,
            _ => SymbolProviderFailureCode::Identity,
        }
    }
}

impl fmt::Debug for ProviderCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.code().fmt(f)
    }
}
impl fmt::Display for ProviderCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Symbol provider {:?}", self.code())
    }
}
impl Error for ProviderCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Control(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            Self::Selection(cause) => Some(cause),
            Self::Facts(cause) => Some(cause),
            Self::Utf16(cause) => Some(cause),
            Self::Reserve(cause) => Some(cause),
            _ => None,
        }
    }
}
