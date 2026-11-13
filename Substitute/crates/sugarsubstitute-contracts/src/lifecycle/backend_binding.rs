// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate admitted backend selections without granting native or process authority.

/// SHA256 of the existing native backend identity owner, or a tagged canonical attachment.
///
/// Managed values originate only after the claims owner resolves actual root/interpreter
/// objects, account, arguments and port. The digest conveys matching data only. A fresh lease,
/// peer verification, origin and native custody remain mandatory on every launch and I/O.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendBindingDigest([u8; 32]);
impl BackendBindingDigest {
    /// Retain exactly one SHA256 observation; construction authenticates no source.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Borrow the exact fixed-width digest for authenticated transport or local correlation.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
