// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the exact authored request scope of an actual original-cutoff rejection.
use super::*;
/// A direct deadline rejection from the real ledger, preserving header, purpose and cutoff.
/// This grants neither receipt admission nor ordinary payload/native authority.
#[derive(Debug)]
pub(in crate::production::child) struct RequestExpiry {
    pub(in crate::production::child) header: WireHeader,
    pub(in crate::production::child) original: Moment,
    pub(in crate::production::child) purpose: Purpose,
    cause: RequestError,
}
impl fmt::Display for RequestExpiry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original correlated request cutoff expired")
    }
}
impl Error for RequestExpiry {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
impl ClientRequests {
    /// Project only an actual direct Deadline from this ledger's finite acquisition/output owner.
    /// Foreign, ordinary, malformed and ordering errors retain their original rejection.
    pub(in crate::production::child) fn expiry(
        &self,
        header: WireHeader,
        cause: RequestError,
    ) -> Result<RequestExpiry, RequestError> {
        if cause != RequestError::Deadline {
            return Err(cause);
        }
        let p = self.find(header)?;
        if p.purpose == Purpose::Other {
            return Err(cause);
        }
        Ok(RequestExpiry { header, original: p.original, purpose: p.purpose, cause })
    }
}
