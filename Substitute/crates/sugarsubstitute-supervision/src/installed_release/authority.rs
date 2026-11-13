// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep selected native effects and cleanup with their exact sealed execution origin.
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext};
use sugarsubstitute_execution::{
    CaptureBundleOrigin, CaptureBundleRetirement, CommittedReaderOrigin, CommittedReaderRetirement,
    SymbolBundleOrigin, SymbolBundleRetirement,
};

#[derive(Clone)]
pub(crate) enum ReleaseAuthority {
    Capture(CaptureBundleOrigin),
    Symbol(SymbolBundleOrigin),
    CommittedReader(crate::committed_reader::authority::ReaderAuthority),
}
impl ReleaseAuthority {
    pub(crate) fn original_context(&self) -> Result<&JobContext, AdmissionError> {
        match self {
            Self::Capture(origin) => origin.original_context(),
            Self::Symbol(origin) => origin.original_context(),
            Self::CommittedReader(origin) => origin.original_context(),
        }
    }
    pub(crate) fn check_effect(&self) -> Result<(), AdmissionError> {
        match self {
            Self::Capture(origin) => origin.check_effect(),
            Self::Symbol(origin) => origin.check_effect(),
            Self::CommittedReader(origin) => origin.check_effect(),
        }
    }
    pub(crate) fn control_storage_bytes(&self) -> usize {
        match self {
            Self::Capture(origin) => origin.control_storage_bytes(),
            Self::Symbol(origin) => origin.control_storage_bytes(),
            Self::CommittedReader(origin) => origin.control_storage_bytes(),
        }
    }
    pub(in crate::installed_release) fn capture(&self) -> Option<&CaptureBundleOrigin> {
        match self {
            Self::Capture(origin) => Some(origin),
            Self::Symbol(_) | Self::CommittedReader(_) => None,
        }
    }
    pub(in crate::installed_release) fn symbol(&self) -> Option<&SymbolBundleOrigin> {
        match self {
            Self::Symbol(origin) => Some(origin),
            Self::Capture(_) | Self::CommittedReader(_) => None,
        }
    }
    pub(crate) fn reader(&self) -> Option<&CommittedReaderOrigin> {
        match self {
            Self::CommittedReader(origin) => origin.origin().ok(),
            _ => None,
        }
    }
}

pub(crate) enum SelectedRetirement<'a> {
    Capture(&'a CaptureBundleRetirement),
    Symbol(&'a SymbolBundleRetirement),
    CommittedReader(&'a CommittedReaderRetirement),
}
impl SelectedRetirement<'_> {
    pub(crate) fn matches_origin(&self, origin: &ReleaseAuthority) -> bool {
        match (self, origin) {
            (Self::Capture(proof), ReleaseAuthority::Capture(origin)) => {
                proof.matches_origin(origin)
            }
            (Self::Symbol(proof), ReleaseAuthority::Symbol(origin)) => proof.matches_origin(origin),
            (Self::CommittedReader(proof), ReleaseAuthority::CommittedReader(origin)) => {
                origin.origin().is_ok_and(|origin| proof.matches_origin(origin))
            }
            _ => false,
        }
    }
    pub(crate) fn check(&self) -> Result<(), AdmissionError> {
        match self {
            Self::Capture(proof) => proof.check(),
            Self::Symbol(proof) => proof.check(),
            Self::CommittedReader(proof) => proof.check(),
        }
    }
}
