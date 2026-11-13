// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence one existing Main's native recovery presentation without backend or page authority.
use crate::{DesktopStartupObservation, MainBackendState, MainIdentity, MainRecoveryLabels};
use sugarsubstitute_contracts::lifecycle::LeaseKey;

/// Immutable current native action; the local revision never crosses the host boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetryAction {
    pub(crate) identity: MainIdentity,
    pub(crate) failed: LeaseKey,
    revision: u64,
}
/// Preallocated button state; labels stay borrowed from the canonical localization owner.
#[derive(Clone, Copy)]
pub(crate) struct RetryPresentation {
    #[cfg(target_os = "windows")]
    pub(crate) identity: MainIdentity,
    pub(crate) visible: bool,
    pub(crate) label: &'static str,
    pub(crate) action: Option<RetryAction>,
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
    /// Borrow canonical labels and the fixed current native visibility/action snapshot.
    pub(crate) fn presentation(&self, labels: MainRecoveryLabels) -> RetryPresentation {
        let visible = !self.closed
            && self.outage.is_some_and(|(_, state)| {
                matches!(state, MainBackendState::RetryReady | MainBackendState::Retrying)
            });
        RetryPresentation {
            #[cfg(target_os = "windows")]
            identity: self.identity,
            visible,
            label: if self.action().is_some() { labels.retry_ready } else { labels.retry },
            action: self.action(),
        }
    }
    /// Closing and revision exhaustion permanently suppress every retained action.
    pub(crate) fn close(&mut self) {
        self.closed = true;
    }
}
#[cfg(test)]
#[path = "main_backend_tests.rs"]
mod tests;
