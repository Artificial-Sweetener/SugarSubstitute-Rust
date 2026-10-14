// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match independent backend custody sessions without granting native ownership.
use crate::lifecycle::{CoordinatorKey, IdentityError};
use std::num::NonZeroU64;
mod codec;
pub use codec::{
    BACKEND_CUSTODY_FRAME_BYTES, BackendCustodyCommand, BackendCustodyFrame, BackendCustodyReceipt,
    BackendCustodyWireError,
};

/// Distinguish one custodian from every prior session of the same coordinator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendCustodySession(NonZeroU64);
impl BackendCustodySession {
    /// Validate a positive session before retaining its matching facts.
    /// # Errors
    /// Zero returns [`IdentityError::Zero`].
    pub const fn new(value: u64) -> Result<Self, IdentityError> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(IdentityError::Zero),
        }
    }
    /// Return the positive session; authentication remains with the native transport.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
    /// Advance once without wrapping or changing the current session.
    /// # Errors
    /// The largest identity returns [`IdentityError::Exhausted`].
    pub fn next(self) -> Result<Self, IdentityError> {
        Self::new(self.get().checked_add(1).ok_or(IdentityError::Exhausted)?)
    }
}

/// Bind a custody session to the complete installation/account and owner epoch.
/// This value supplies correlation facts. It cannot open a Job, file or process.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendCustodyKey {
    owner: CoordinatorKey,
    session: BackendCustodySession,
}
impl BackendCustodyKey {
    /// Retain validated matching facts; the transport establishes provenance.
    pub const fn new(owner: CoordinatorKey, session: BackendCustodySession) -> Self {
        Self { owner, session }
    }
    /// Return the complete coordinator identity without native authority.
    pub const fn owner(self) -> CoordinatorKey {
        self.owner
    }
    /// Return the custodian session within this exact owner lifetime.
    pub const fn session(self) -> BackendCustodySession {
        self.session
    }
}
