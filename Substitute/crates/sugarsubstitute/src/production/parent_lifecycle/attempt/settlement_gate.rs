// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join actual Main and backend retirement before staging the original settlement ACK.
use super::super::{ParentError, ParentLifecycle, Reply, Result};
use sugarsubstitute_contracts::{execution::Moment, lifecycle::LeaseKey};

/// Facts from the existing trusted backend adapter for the exact retained lease.
/// Test boundaries replace that adapter query, not the Parent gate or Main owner.
pub(in super::super) struct BackendRetirement {
    pub(in super::super) cleanup_failed: bool,
    pub(in super::super) released: bool,
}

impl ParentLifecycle {
    /// A fresh observation queries only the retained exact lease. Neither a request,
    /// timeout nor a native Main negative fact alone makes backend custody settled.
    pub(in super::super) fn observe_attempt_retirement(
        &mut self,
        now: Moment,
        inspect: impl FnOnce(Option<LeaseKey>) -> Result<BackendRetirement>,
    ) -> Result<Option<usize>> {
        if self.attempt_cleanup.is_none() || self.expire_attempt(now)? {
            return Ok(None);
        }
        let attempt = self.attempt_cleanup.as_ref().ok_or(ParentError::WrongPhase)?;
        if attempt.ready || attempt.failed.is_some() || !attempt.main_retired(self.main.stage()) {
            return Ok(None);
        }
        let facts = inspect(attempt.lease)?;
        if facts.cleanup_failed {
            self.requests.get_mut(attempt.index)?.reply = Some(Reply::Failed(
                sugarsubstitute_contracts::lifecycle::LifecycleFailureCode::Cleanup,
            ));
            self.attempt_cleanup.as_mut().ok_or(ParentError::WrongPhase)?.failed =
                Some(super::AttemptCleanupFailure::Backend);
            return Ok(None);
        }
        Ok(facts.released.then_some(attempt.index))
    }

    /// Caller has already joined the real backend result and retained its original
    /// cause. Staging ACK preserves acquisition and request state until actual write.
    pub(in super::super) fn stage_settlement_reply(&mut self, index: usize) -> Result<()> {
        let attempt = self.attempt_cleanup.as_mut().ok_or(ParentError::WrongPhase)?;
        if attempt.index != index || attempt.ready || attempt.failed.is_some() {
            return Err(ParentError::WrongPhase.into());
        }
        self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
        attempt.ready = true;
        Ok(())
    }
}
