// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish scoped lifecycle facts from authentication and native authority.
use std::{error::Error, fmt, num::NonZeroU64};

/// Reject invalid lifecycle identities without execution-owner terminology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// Numeric registry identities must be positive.
    Zero,
    /// A numeric identity or generation cannot advance.
    Exhausted,
    /// Fresh coordinator epochs cannot contain only zero bytes.
    InvalidEpoch,
}
impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid lifecycle identity: {self:?}")
    }
}
impl Error for IdentityError {}

/// Canonical installation/account digest supplied by the native resolver.
/// Bytes are matching facts, not evidence of canonical resolution or authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NamespaceId([u8; 32]);
impl NamespaceId {
    /// Retain an exact digest without hashing paths or trusting their provenance.
    pub const fn new(value: [u8; 32]) -> Self {
        Self(value)
    }
    /// Borrow the immutable matching digest.
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
/// Fresh cryptographically generated coordinator epoch supplied by its native owner.
/// Construction validates shape only; callers establish freshness and provenance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CoordinatorEpoch([u8; 16]);
impl CoordinatorEpoch {
    /// Reject the reserved all-zero epoch without inventing native randomness.
    pub fn new(value: [u8; 16]) -> Result<Self, IdentityError> {
        if value.iter().all(|byte| *byte == 0) {
            Err(IdentityError::InvalidEpoch)
        } else {
            Ok(Self(value))
        }
    }
    /// Borrow the exact fresh lifetime bytes.
    pub const fn bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroU64);
        impl $name {
            /// Construct a positive identity; zero returns `IdentityError::Zero`.
            pub const fn new(value: u64) -> Result<Self, IdentityError> {
                match NonZeroU64::new(value) {
                    Some(value) => Ok(Self(value)),
                    None => Err(IdentityError::Zero),
                }
            }
            /// Return the identity value, granting no native privilege.
            pub const fn get(self) -> u64 {
                self.0.get()
            }
        }
    };
}
identity!(InstanceId, "Identify an independent desktop instance within its coordinator.");
identity!(RunGeneration, "Identify a fresh desktop run, separately from surface revisions.");
identity!(ResourceId, "Identify one backend resource within its coordinator.");
identity!(ResourceGeneration, "Identify one acquisition lifetime of a backend resource.");
identity!(ConnectionId, "Identify an adapter-authenticated connection within one desktop run.");
identity!(LeaseId, "Identify a connection-bound backend lease.");
identity!(RequestId, "Identify a monotonically assigned operation within one desktop run.");
identity!(ProcessId, "Identify an entry in the native registry, never an operating-system PID.");
identity!(ShutdownId, "Identify one acknowledged shutdown transaction within a run.");

impl RunGeneration {
    /// Advance without wrapping; exhaustion preserves the current generation.
    pub fn next(self) -> Result<Self, IdentityError> {
        Self::new(self.get().checked_add(1).ok_or(IdentityError::Exhausted)?)
    }
}
impl ResourceGeneration {
    /// Advance without wrapping; a settled predecessor remains a distinct lifetime.
    pub fn next(self) -> Result<Self, IdentityError> {
        Self::new(self.get().checked_add(1).ok_or(IdentityError::Exhausted)?)
    }
}

/// Scope facts to a coordinator lifetime; these fields are not a capability.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CoordinatorKey {
    /// Installation/account coordination namespace.
    pub namespace: NamespaceId,
    /// Fresh owner lifetime.
    pub epoch: CoordinatorEpoch,
}
/// Scope one desktop generation to its owner and instance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RunKey {
    /// Current coordinator identity.
    pub owner: CoordinatorKey,
    /// Independent desktop identity.
    pub instance: InstanceId,
    /// Fresh run generation.
    pub generation: RunGeneration,
}
/// Scope one backend acquisition generation to its coordinator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceKey {
    /// Resource lifecycle owner.
    pub owner: CoordinatorKey,
    /// Logical backend identity.
    pub id: ResourceId,
    /// Current native acquisition generation.
    pub generation: ResourceGeneration,
}
/// Reference a retained native registry entry without carrying a handle or PID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProcessKey {
    /// Native registry owner, shared by desktop and backend process entries.
    pub owner: CoordinatorKey,
    /// Exact process registry identity.
    pub id: ProcessId,
}
/// Identify one lease without granting control of another connection's backend.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LeaseKey {
    /// Run holding the lease.
    pub run: RunKey,
    /// Backend acquisition lifetime.
    pub resource: ResourceKey,
    /// Authenticated connection binding.
    pub connection: ConnectionId,
    /// Unique coordinator-assigned lease identity.
    pub id: LeaseId,
}
