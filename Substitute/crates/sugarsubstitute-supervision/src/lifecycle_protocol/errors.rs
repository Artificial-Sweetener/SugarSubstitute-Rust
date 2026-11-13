// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve bounded framing failures without formatting secrets or launch inputs.
use std::{collections::TryReserveError, error::Error, fmt};
use sugarsubstitute_contracts::configuration::ConfigurationError;

/// Reject a malformed or unsupported frame without native or authentication effects.
#[derive(Debug)]
pub enum ProtocolError {
    /// Raw Windows units are empty, oversized or have an odd byte count.
    RawWindowsPath,
    /// Folder total, offset or chunk count violates the fixed two-fragment partition.
    FragmentShape,
    /// A bounded label violates the shared readiness version grammar.
    VersionLabel,
    /// Output bytes, flags or explicit loss violate the bounded raw-batch shape.
    OutputShape,
    /// The complete frame exceeds the 64 KiB control limit.
    FrameTooLarge,
    /// A fixed field or length-prefixed value is incomplete.
    Truncated,
    /// The declared total length differs from the supplied complete frame.
    FrameLength,
    /// The magic or supported wire version/header size differs.
    Schema,
    /// An operation, payload tag or reserved field is unsupported.
    UnsupportedTag,
    /// Extra bytes remain after the exact operation payload.
    TrailingBytes,
    /// A claimed identity, its kind-specific presence or epoch is invalid.
    Identity,
    /// A raw signed QPC deadline or frequency is not positive.
    Clock,
    /// A counted length or checked size is outside the supported bounds.
    Length,
    /// UTF-16 contains an unmatched surrogate; replacement is forbidden.
    InvalidUtf16,
    /// A launch path or argument contains a NUL code unit.
    InteriorNul,
    /// The existing launch/profile/origin domain rejects this exact input.
    Configuration(ConfigurationError),
    /// The caller-owned output cannot hold the already-preflighted complete frame.
    OutputTooSmall {
        /// Required complete frame size in bytes.
        required: usize,
        /// Actual caller-provided capacity in bytes.
        available: usize,
    },
    /// An explicitly requested domain conversion could not reserve storage.
    Allocation(TryReserveError),
}
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Only typed classifications and capacities appear, never the payload.
        match self {
            Self::Configuration(_) => f.write_str("Lifecycle protocol domain input rejected"),
            Self::Allocation(_) => f.write_str("Lifecycle protocol allocation failed"),
            other => write!(f, "Lifecycle protocol input rejected: {other:?}"),
        }
    }
}
impl Error for ProtocolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(cause) => Some(cause),
            Self::Allocation(cause) => Some(cause),
            _ => None,
        }
    }
}
