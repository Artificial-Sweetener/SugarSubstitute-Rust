// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence one existing Main's backend recovery without backend or page authority.
use crate::{DesktopStartupObservation, MainBackendState, MainIdentity};
use sugarsubstitute_contracts::{host_backend::BackendStatus, lifecycle::LeaseKey};

/// Immutable current native action; the local revision never crosses the host boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetryAction {
    pub(crate) identity: MainIdentity,
    pub(crate) failed: LeaseKey,
    revision: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryError {
    Identity,
    StaleLease,
    Closed,
    RevisionExhausted,
}
impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Native Main recovery rejected: {self:?}")
    }
}
impl std::error::Error for RecoveryError {}

/// One fixed outage slot preserves the completed failed lease as a replay tombstone.
pub(crate) struct Recovery {
    identity: MainIdentity,
    outage: Option<(LeaseKey, MainBackendState)>,
    revision: u64,
    closed: bool,
}
impl Recovery {
    /// Begin hidden/disabled with no failed lease or retry action.
    pub(crate) fn new(identity: MainIdentity) -> Self {
        Self { identity, outage: None, revision: 0, closed: false }
    }
    /// Reject foreign/current-lease replacement before changing any presentation state.
    pub(crate) fn present(
        &mut self,
        identity: MainIdentity,
        failed: LeaseKey,
        state: MainBackendState,
    ) -> Result<bool, RecoveryError> {
        self.check(identity, failed)?;
        match self.outage {
            Some((previous, old)) if previous == failed => {
                if old == state {
                    return Ok(false);
                }
                if old == MainBackendState::Recovered {
                    return Err(RecoveryError::StaleLease);
                }
            }
            Some((_, MainBackendState::Recovered)) if state != MainBackendState::Recovered => {}
            None if state != MainBackendState::Recovered => {}
            _ => return Err(RecoveryError::StaleLease),
        }
        self.advance()?;
        self.outage = Some((failed, state));
        Ok(true)
    }
    fn check(&self, identity: MainIdentity, failed: LeaseKey) -> Result<(), RecoveryError> {
        if identity != self.identity
            || failed.run != identity.run
            || failed.resource.owner != identity.run.owner
        {
            return Err(RecoveryError::Identity);
        }
        if self.closed {
            return Err(RecoveryError::Closed);
        }
        Ok(())
    }
    /// Consume exact Root-supplied handoff lineage without claiming a healthy replacement.
    pub(crate) fn replace(
        &mut self,
        identity: MainIdentity,
        previous: LeaseKey,
        failed: LeaseKey,
        state: MainBackendState,
    ) -> Result<bool, RecoveryError> {
        self.check(identity, failed)?;
        self.check(identity, previous)?;
        if state == MainBackendState::Recovered
            || previous == failed
            || previous.connection != failed.connection
            || self.outage.is_none_or(|(current, old)| {
                current != previous || old == MainBackendState::Recovered
            })
        {
            return Err(RecoveryError::StaleLease);
        }
        self.advance()?;
        self.outage = Some((failed, state));
        Ok(true)
    }
    fn advance(&mut self) -> Result<(), RecoveryError> {
        match self.revision.checked_add(1) {
            Some(next) => {
                self.revision = next;
                Ok(())
            }
            None => {
                self.closed = true;
                Err(RecoveryError::RevisionExhausted)
            }
        }
    }
    /// What the page is told: how the backend stands and the revision that names this state.
    pub(crate) fn page_state(&self) -> (u64, BackendStatus) {
        let status = match self.outage {
            _ if self.closed => BackendStatus::Running,
            None | Some((_, MainBackendState::Recovered)) => BackendStatus::Running,
            Some((_, MainBackendState::Retiring)) => BackendStatus::Stopped,
            Some((_, MainBackendState::RetryReady)) => BackendStatus::RetryReady,
            Some((_, MainBackendState::Retrying)) => BackendStatus::Retrying,
        };
        (self.revision, status)
    }
    /// The retry the page may ask for by naming `revision`, the state it was shown.
    pub(crate) fn action_at(&self, revision: u64) -> Option<RetryAction> {
        self.action().filter(|action| action.revision == revision)
    }
    /// Native callbacks carry this whole immutable tuple, never id4 alone.
    pub(crate) fn action(&self) -> Option<RetryAction> {
        let (failed, MainBackendState::RetryReady) = self.outage? else {
            return None;
        };
        (!self.closed).then_some(RetryAction {
            identity: self.identity,
            failed,
            revision: self.revision,
        })
    }
    /// Consume exactly one still-current click before the host can observe it.
    pub(crate) fn consume(
        &mut self,
        action: RetryAction,
    ) -> Result<Option<DesktopStartupObservation>, RecoveryError> {
        if self.action() != Some(action) {
            return Ok(None);
        }
        self.advance()?;
        self.outage = Some((action.failed, MainBackendState::Retrying));
        Ok(Some(DesktopStartupObservation::BackendRetryRequested {
            identity: action.identity,
            failed: action.failed,
        }))
    }
    /// Closing and revision exhaustion permanently suppress every retained action.
    pub(crate) fn close(&mut self) {
        self.closed = true;
    }
}
#[cfg(test)]
#[path = "main_backend_tests.rs"]
mod tests;
