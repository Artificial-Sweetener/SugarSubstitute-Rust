// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the genuine executor, wire and transcript causes behind bounded categories.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, MemoryClaimError, TerminalKind},
    transcript::TranscriptError,
};
use sugarsubstitute_supervision::lifecycle_protocol::ProtocolError;
/// Reject output without acknowledging delivery or formatting foreign bytes.
#[derive(Debug)]
pub(crate) enum OutputError {
    /// Central finite-job admission failed before the parser ran.
    Admission(AdmissionError),
    /// Original job reservation cannot fund the declared source/scratch bound.
    Claim(MemoryClaimError),
    /// Genuine closed wire decoding failed.
    Protocol(ProtocolError),
    /// Actual transcript sequence, EOF or snapshot admission failed.
    Transcript(TranscriptError),
    /// Header, lease, delivery or original job identity differs.
    Identity,
    /// A pull/job/frame already occupies the sole slot, or admission closed.
    Order,
    /// The actual original job was cancelled before a parser effect.
    Cancelled,
    /// The original finite cutoff expired.
    Deadline,
    /// Retained mutable state was poisoned; original frame remains held.
    Poisoned,
    /// Actual finite worker ended without a success value.
    Terminal(TerminalKind),
}
impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Terminal(kind) => write!(f, "Production output worker ended with {kind:?}"),
            _ => f.write_str("Production output operation failed"),
        }
    }
}
impl Error for OutputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Claim(e) => Some(e),
            Self::Protocol(e) => Some(e),
            Self::Transcript(e) => Some(e),
            _ => None,
        }
    }
}
impl From<TranscriptError> for OutputError {
    fn from(e: TranscriptError) -> Self {
        Self::Transcript(e)
    }
}
impl From<ProtocolError> for OutputError {
    fn from(e: ProtocolError) -> Self {
        Self::Protocol(e)
    }
}
