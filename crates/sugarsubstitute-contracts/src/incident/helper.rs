// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish diagnostic sessions from complete durable operation identities.
use super::IncidentError;
use crate::lifecycle::CoordinatorKey;
use std::num::NonZeroU64;

/// Select one separately supervised diagnostic family role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticRole {
    /// Perform one finite protected-storage operation.
    Storage,
    /// Resolve one actual captured exception frame.
    Symbolize,
    /// Present an independently owned native incident report.
    Reporter,
}

/// Identify one diagnostic family session within its exact coordinator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticSessionId(NonZeroU64);
impl DiagnosticSessionId {
    /// Validate a positive session identity without admitting a process.
    /// # Errors
    /// Zero returns [`IncidentError::InvalidIdentity`].
    pub const fn new(value: u64) -> Result<Self, IncidentError> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(IncidentError::InvalidIdentity),
        }
    }
    /// Observe the exact positive identity; this grants no native authority.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
    /// Advance once, preserving the original value on exhaustion.
    /// # Errors
    /// The maximum identity returns [`IncidentError::OperationExhausted`].
    pub fn next(self) -> Result<Self, IncidentError> {
        Self::new(self.get().checked_add(1).ok_or(IncidentError::OperationExhausted)?)
    }
}

/// Bind a diagnostic session to its complete coordinator and role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticKey {
    owner: CoordinatorKey,
    role: DiagnosticRole,
    session: DiagnosticSessionId,
}
impl DiagnosticKey {
    /// Combine already validated matching facts; authentication remains external.
    pub const fn new(
        owner: CoordinatorKey,
        role: DiagnosticRole,
        session: DiagnosticSessionId,
    ) -> Self {
        Self { owner, role, session }
    }
    /// Observe the exact installation/account namespace and coordinator epoch.
    pub const fn owner(self) -> CoordinatorKey {
        self.owner
    }
    /// Observe the role that owns this session's quota and lifecycle.
    pub const fn role(self) -> DiagnosticRole {
        self.role
    }
    /// Observe the positive family session identity.
    pub const fn session(self) -> DiagnosticSessionId {
        self.session
    }
}

/// Identify one logical storage operation independently of helper relaunches.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IncidentOperationId(NonZeroU64);
impl IncidentOperationId {
    /// Validate a positive operation identity without admitting an effect.
    /// # Errors
    /// Zero returns [`IncidentError::InvalidIdentity`].
    pub const fn new(value: u64) -> Result<Self, IncidentError> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(IncidentError::InvalidIdentity),
        }
    }
    /// Observe the exact positive operation identity.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
    /// Advance without wrapping or changing the original operation.
    /// # Errors
    /// The maximum identity returns [`IncidentError::OperationExhausted`].
    pub fn next(self) -> Result<Self, IncidentError> {
        Self::new(self.get().checked_add(1).ok_or(IncidentError::OperationExhausted)?)
    }
}

/// Preserve an operation's complete original owner across durable recovery.
///
/// A historical journal and its current recovery operation have distinct keys,
/// even when their positive numeric IDs and installation namespace match.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogicalStoreOperationKey {
    owner: CoordinatorKey,
    id: IncidentOperationId,
}
impl LogicalStoreOperationKey {
    /// Combine validated facts without rebinding a historical operation.
    pub const fn new(owner: CoordinatorKey, id: IncidentOperationId) -> Self {
        Self { owner, id }
    }
    /// Observe the complete original coordinator identity.
    pub const fn owner(self) -> CoordinatorKey {
        self.owner
    }
    /// Observe the exact positive operation ID within that coordinator.
    pub const fn id(self) -> IncidentOperationId {
        self.id
    }
}
