// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate live operation authority correlation from inert historical identities.
use super::IncidentOperationBudget;
use sugarsubstitute_contracts::incident::{
    IncidentKey, IncidentRevision, LogicalStoreOperationKey,
};

/// Coordinator-produced live correlation; constructors remain with the service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncidentOperationScope {
    pub(super) incident: IncidentKey,
    pub(super) expected_revision: IncidentRevision,
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) original: IncidentOperationBudget,
}
impl IncidentOperationScope {
    /// Return the full subject.
    pub const fn incident(self) -> IncidentKey {
        self.incident
    }
    /// Return the exact predecessor revision for compare-and-swap.
    pub const fn expected_revision(self) -> IncidentRevision {
        self.expected_revision
    }
    /// Return the actual current logical operation identity.
    pub const fn operation(self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Return the immutable original interval.
    pub const fn original(self) -> IncidentOperationBudget {
        self.original
    }
}
/// One current coordinator recovery admission; historical moments are never constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncidentRecoveryScope {
    pub(super) operation: LogicalStoreOperationKey,
    pub(super) original: IncidentOperationBudget,
}
impl IncidentRecoveryScope {
    /// Return the current recovery operation.
    pub const fn operation(self) -> LogicalStoreOperationKey {
        self.operation
    }
    /// Return its original interval.
    pub const fn original(self) -> IncidentOperationBudget {
        self.original
    }
}
/// Match already-issued recovered-source retirement independently of recovery closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveredSourceScope {
    pub(super) recovery: IncidentRecoveryScope,
    pub(super) historical_operation: LogicalStoreOperationKey,
    pub(super) incident: IncidentKey,
    pub(super) expected_revision: IncidentRevision,
}
impl RecoveredSourceScope {
    /// Return the actual current recovery admission.
    pub const fn recovery(self) -> IncidentRecoveryScope {
        self.recovery
    }
    /// Return inert original publication identity without rebinding its epoch.
    pub const fn historical_operation(self) -> LogicalStoreOperationKey {
        self.historical_operation
    }
    /// Return the historical subject.
    pub const fn incident(self) -> IncidentKey {
        self.incident
    }
    /// Return the originally projected revision.
    pub const fn expected_revision(self) -> IncidentRevision {
        self.expected_revision
    }
}
/// Retain source settlement correlation without granting cleanup capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncidentSourceScope {
    /// Original live failure/publication operation and interval.
    Live(IncidentOperationScope),
    /// Current recovery admission plus inert historical matching facts.
    Recovered(RecoveredSourceScope),
}
impl IncidentSourceScope {
    /// Locate the subject correlated by either live or recovered source custody.
    pub(super) const fn incident(self) -> IncidentKey {
        match self {
            Self::Live(scope) => scope.incident(),
            Self::Recovered(scope) => scope.incident(),
        }
    }
}
