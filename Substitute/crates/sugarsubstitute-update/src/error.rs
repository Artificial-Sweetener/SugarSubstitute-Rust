// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve authentication causes without accepting transport claims as update authority.
/// Update trust could not authorize the supplied metadata or package bytes.
#[derive(Debug)]
pub enum Error {
    /// TUF rejected a signature, role, root transition or metadata binding.
    Trust(tuf::Error),
    /// A checkpoint could not be decoded; its original bytes must be preserved.
    Encoding(serde_json::Error),
    /// Authenticated descriptor violates product routing, release history or format policy.
    Offer(sugarsubstitute_contracts::update::Rejection),
    /// Input exceeds a fixed bound or a byte counter overflowed.
    Bound,
    /// Persisted state has an unknown schema or inconsistent fields.
    State,
    /// A metadata version or its dependent role regressed.
    Rollback,
    /// Different bytes reused an already accepted role version.
    Equivocation,
    /// Metadata has expired at the supplied trusted wall-clock time.
    Expired,
    /// The supplied time predates a previously recorded successful verification.
    ClockRollback,
    /// No complete authenticated metadata set is available for target lookup.
    Incomplete,
    /// The authenticated offer requires an engine/installer update or is already current.
    Admission,
    /// A target has no supported digest or its streamed bytes do not match.
    Digest,
}
impl Error {
    /// Whether verification failed only because signed metadata outlived its lifetime, as
    /// opposed to a signature, rollback, routing or storage problem.
    pub fn is_expiry(&self) -> bool {
        matches!(self, Self::Expired | Self::Trust(tuf::Error::ExpiredMetadata { .. }))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trust(_) => f.write_str("Update metadata authentication failed"),
            Self::Encoding(_) => f.write_str("Update trust checkpoint could not be decoded"),
            Self::Offer(_) => f.write_str("Authenticated update offer was rejected"),
            other => write!(f, "Update verification rejected: {other:?}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Trust(cause) => Some(cause),
            Self::Encoding(cause) => Some(cause),
            Self::Offer(cause) => Some(cause),
            _ => None,
        }
    }
}
impl From<tuf::Error> for Error {
    fn from(cause: tuf::Error) -> Self {
        Self::Trust(cause)
    }
}
impl From<serde_json::Error> for Error {
    fn from(cause: serde_json::Error) -> Self {
        Self::Encoding(cause)
    }
}
