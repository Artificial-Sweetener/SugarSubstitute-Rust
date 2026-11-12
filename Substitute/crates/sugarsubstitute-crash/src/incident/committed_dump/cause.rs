// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain original native/serde/Core causes beneath categorical public observations.
use super::super::IncidentStoreError;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{AdmissionError, PanicCause};
use sugarsubstitute_supervision::ClaimError;
/// Categorical copied observation; it grants neither read nor cleanup authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DumpReadCode {
    /// Original source/native operation failed.
    Native,
    /// Actual Core cancellation/deadline/clock/admission rejected work.
    Control,
    /// Strict protected schema or domain facts were invalid.
    Record,
    /// Same copied inode did not contain the exact Crashpad subject/build/UUID.
    Identity,
    /// A fixed original allocation or format bound was exceeded.
    Capacity,
    /// A callback panicked; original bounded panic description remains owned.
    Panicked,
}
pub(crate) enum DumpReadCause {
    Native(ClaimError),
    Control(AdmissionError),
    Record(IncidentStoreError),
    Parser(crate::native::committed_parser::CommittedParserError),
    Reserve(std::collections::TryReserveError),
    Identity,
    Capacity,
    Panicked(PanicCause),
}
impl DumpReadCause {
    pub(super) fn code(&self) -> DumpReadCode {
        match self {
            Self::Native(_) => DumpReadCode::Native,
            Self::Control(_) => DumpReadCode::Control,
            Self::Record(_) => DumpReadCode::Record,
            Self::Parser(_) => DumpReadCode::Identity,
            Self::Reserve(_) | Self::Capacity => DumpReadCode::Capacity,
            Self::Identity => DumpReadCode::Identity,
            Self::Panicked(_) => DumpReadCode::Panicked,
        }
    }
}
/// Borrow an actual retained failure; paths/record bytes are excluded from diagnostics.
pub struct DumpReadFailure<'a>(pub(super) &'a DumpReadCause);
impl DumpReadFailure<'_> {
    /// Return a categorical observation without copying the underlying failure.
    pub fn code(&self) -> DumpReadCode {
        self.0.code()
    }
    /// Borrow the actual bounded callback panic without copying its allocation.
    pub fn panic_cause(&self) -> Option<&PanicCause> {
        match self.0 {
            DumpReadCause::Panicked(cause) => Some(cause),
            _ => None,
        }
    }
}
impl fmt::Debug for DumpReadFailure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DumpReadFailure").field("code", &self.code()).finish()
    }
}
impl fmt::Display for DumpReadFailure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Committed dump validation failed: {:?}", self.code())
    }
}
impl Error for DumpReadFailure<'_> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.0 {
            DumpReadCause::Native(cause) => Some(cause),
            DumpReadCause::Control(cause) => Some(cause),
            DumpReadCause::Record(cause) => Some(cause),
            DumpReadCause::Parser(cause) => Some(cause),
            DumpReadCause::Reserve(cause) => Some(cause),
            _ => None,
        }
    }
}

impl fmt::Debug for DumpReadCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.code().fmt(f)
    }
}
impl fmt::Display for DumpReadCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Committed dump {:?}", self.code())
    }
}
impl Error for DumpReadCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Native(cause) => Some(cause),
            Self::Control(cause) => Some(cause),
            Self::Record(cause) => Some(cause),
            Self::Parser(cause) => Some(cause),
            Self::Reserve(cause) => Some(cause),
            _ => None,
        }
    }
}
