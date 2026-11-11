// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe fixed native identity facts without native handles or proof constructors.

use super::{EntryRejection, EntryShapeError};
use crate::lifecycle::{CoordinatorKey, NamespaceId, RunKey};
use std::fmt;

macro_rules! positive_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name(u64);
        impl $name {
            /// Validate an inert positive identity without allocating an authority.
            /// # Errors
            /// Returns [`EntryShapeError::Identity`] for zero.
            pub const fn new(value: u64) -> Result<Self, EntryShapeError> {
                if value == 0 { Err(EntryShapeError::Identity) } else { Ok(Self(value)) }
            }
            /// Return the exact positive value; allocation must never wrap.
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}
positive_id!(EntryConnectionId, "Identify one retained entry transport connection.");
positive_id!(EntryOperationId, "Identify one bounded correlated entry transport operation.");
positive_id!(EntryRequestId, "Identify one original immutable desktop invocation request.");

/// Inert exact account/session/installation facts for independent adapter verification.
/// Caller construction grants no native peer, namespace, election or coordinator proof.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EntryNativeIdentityFacts {
    namespace: NamespaceId,
    sid: [u8; 68],
    sid_len: u8,
    session: u32,
    image: [u8; 24],
}
impl EntryNativeIdentityFacts {
    /// Retain a structurally exact revision-one SID and fixed file identity bytes.
    /// # Errors
    /// Rejects malformed SID length, revision or subauthority count. Native account
    /// and image authentication remain the execution adapter's independent checks.
    pub fn new(
        namespace: NamespaceId,
        account_sid: &[u8],
        session_id: u32,
        host_image_identity: [u8; 24],
    ) -> Result<Self, EntryShapeError> {
        if account_sid.len() < 8
            || account_sid.len() > 68
            || account_sid[0] != 1
            || account_sid[1] > 15
            || account_sid.len() != 8 + 4 * usize::from(account_sid[1])
        {
            return Err(EntryShapeError::Identity);
        }
        let mut sid = [0; 68];
        sid[..account_sid.len()].copy_from_slice(account_sid);
        let sid_len = u8::try_from(account_sid.len()).map_err(|_| EntryShapeError::Identity)?;
        Ok(Self { namespace, sid, sid_len, session: session_id, image: host_image_identity })
    }
    /// Return the requested namespace without granting election authority.
    pub const fn namespace(&self) -> NamespaceId {
        self.namespace
    }
    /// Borrow the exact original bounded SID bytes.
    pub fn account_sid(&self) -> &[u8] {
        &self.sid[..usize::from(self.sid_len)]
    }
    /// Return the claimed session identity for independent native comparison.
    pub const fn session_id(&self) -> u32 {
        self.session
    }
    /// Return the claimed volume/file identity for independent native comparison.
    pub const fn host_image_identity(&self) -> [u8; 24] {
        self.image
    }
}
impl fmt::Debug for EntryNativeIdentityFacts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryNativeIdentityFacts").finish_non_exhaustive()
    }
}

/// Describe discovery readiness without turning initializing metadata into a grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryDiscoveryPhase {
    /// Only credential-checked discovery is available.
    Initializing,
    /// Actual claim binding has activated a coordinator endpoint.
    Serving(CoordinatorKey),
}
/// Describe a correlated request outcome independently of child startup readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryOutcome {
    /// A contained child was admitted; this fact grants no backend lease.
    Accepted(RunKey),
    /// The original request was rejected with a closed reason.
    Rejected(EntryRejection),
}
