// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Acknowledge acquisition cleanup only from the same retained backend generation.
use super::*;
use sugarsubstitute_contracts::{
    lifecycle::{RequestId, ResourceKey},
    startup::MainIdentity,
};
use sugarsubstitute_supervision::lifecycle_protocol::WireHeader;

#[path = "attempt/settlement_gate.rs"]
mod settlement_gate;
pub(super) use settlement_gate::BackendRetirement;

/// Original terminal attempt classification; backend causes stay with their actual owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttemptCleanupFailure {
    Backend,
    Deadline,
}
pub(super) struct AttemptSettlement {
    pub(super) index: usize,
    pub(super) resource: Option<ResourceKey>,
    pub(super) lease: Option<sugarsubstitute_contracts::lifecycle::LeaseKey>,
    pub(super) origin: WireHeader,
    pub(super) identity: MainIdentity,
    pub(super) until: Moment,
    pub(super) failed: Option<AttemptCleanupFailure>,
    pub(super) main_retirement_required: bool,
    pub(super) retirement: Option<(WireHeader, bool)>,
    pub(super) ready: bool,
}
/// Retain the original producer cause with its exact Main and backend resource after checked settlement.
/// Reporting consumes a semantic snapshot while this owner retains its native cause.
pub(crate) struct SettledAttemptFailure {
    pub(crate) identity: MainIdentity,
    pub(crate) resource: ResourceKey,
    pub(crate) cause: sugarsubstitute_contracts::lifecycle::LifecycleFailure,
}
impl std::fmt::Debug for SettledAttemptFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettledAttemptFailure")
            .field("identity", &self.identity)
            .field("resource", &self.resource)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for SettledAttemptFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Original settled backend failure")
    }
}
impl Error for SettledAttemptFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
impl AttemptSettlement {
    /// Actual authenticated native retirement request; copied metadata is not a proof.
    pub(super) fn observe_retirement(
        &mut self,
        identity: MainIdentity,
        header: WireHeader,
    ) -> std::result::Result<(), ParentError> {
        if identity != self.identity {
            return Err(ParentError::ForeignPeer);
        }
        if self.retirement.is_some_and(|(old, _)| old != header) {
            return Err(ParentError::ChangedDuplicate);
        }
        self.retirement.get_or_insert((header, false));
        Ok(())
    }
    /// Called only after the original actual write receipt validates full completion.
    pub(super) fn retirement_written(&mut self, header: WireHeader) {
        if let Some((original, written)) = &mut self.retirement
            && *original == header
        {
            *written = true;
        }
    }
    /// Settlement publication cannot overtake native/profile retirement or its actual ACK.
    pub(super) fn main_retired(
        &self,
        stage: sugarsubstitute_application::supervision::MainAuthorizationStage,
    ) -> bool {
        stage == sugarsubstitute_application::supervision::MainAuthorizationStage::Retired
            && (!self.main_retirement_required
                || self.retirement.is_some_and(|(_, written)| written))
    }
}
impl ParentLifecycle {
    pub(super) fn begin_attempt_cleanup(
        &mut self,
        index: usize,
        identity: MainIdentity,
        acquisition: RequestId,
        until: Moment,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let binding = self.binding()?;
        let budget = self.prepare_attempt_cleanup(
            index,
            identity,
            acquisition,
            until,
            resources.execution.client.now()?,
            resources.profile.snapshot().pending,
        )?;
        let attempt = self.attempt_cleanup.as_ref().ok_or(ParentError::WrongPhase)?;
        let lease = attempt.lease;
        if attempt.main_retirement_required {
            self.main.retire(identity)?;
            resources.profile.retire_main_attempt(identity)?;
        }
        if let Some(lease) = lease {
            // The authentic settlement request follows actual Child parser/frame
            // retirement. Dispose this passive delivery loan without asserting ACK,
            // EOF or native family closure; those owners still settle independently.
            self.output = None;
            let transition =
                resources.backend.cleanup_attempt(binding, lease, budget, resources.execution)?;
            self.backend_replies(transition, resources)?;
        }
        self.advance_attempt_cleanup(resources)
    }
    /// Retain the exact current acquisition before requesting backend effects. The caller
    /// authenticates the channel; copied matching facts grant no backend/native authority.
    /// Replay eviction cannot destroy this live attempt's original correlation.
    pub(super) fn prepare_attempt_cleanup(
        &mut self,
        index: usize,
        identity: MainIdentity,
        acquisition: RequestId,
        until: Moment,
        now: Moment,
        native_profile_pending: bool,
    ) -> Result<ShutdownBudget> {
        if self.closed || self.cleanup.is_some() || self.attempt_cleanup.is_some() {
            return Err(ParentError::WrongPhase.into());
        }
        let main_retirement_required = self.main.stage()
            == sugarsubstitute_application::supervision::MainAuthorizationStage::RequestedFolder;
        if (self.main.stage()
            != sugarsubstitute_application::supervision::MainAuthorizationStage::Retired
            && !main_retirement_required)
            || self.main_retirement.is_some()
            || self.profile_pending.is_some()
            || native_profile_pending
        {
            return Err(ParentError::WrongPhase.into());
        }
        let budget = attempt_budget(until, now)?;
        let original = self.acquisition.as_ref().ok_or(ParentError::StaleRequest)?;
        let binding = sugarsubstitute_contracts::lifecycle::ClientBinding {
            run: self.run,
            connection: self.connection,
        };
        let lease = original.settlement_lease(
            binding,
            identity,
            acquisition,
            self.lease,
            self.main.snapshot().identity,
        )?;
        let origin = original.header;
        tracing::debug!(operation="startup_attempt_cleanup",request=?origin.request,run=?self.run,
            original_acquisition_until=?original.original,identity=?identity,lease=?lease,
            "Current acquisition retained independently of bounded replay history");
        let resource = lease.map(|lease| lease.resource);
        self.attempt_cleanup = Some(AttemptSettlement {
            index,
            resource,
            lease,
            until,
            origin,
            identity,
            failed: None,
            main_retirement_required,
            retirement: None,
            ready: false,
        });
        Ok(budget)
    }
    /// Retain expired correlation and all native owners while keeping the transport live.
    /// No reply may be admitted after its unchanged cutoff, even if native work settles later.
    pub(super) fn expire_attempt(&mut self, now: Moment) -> Result<bool> {
        let Some(attempt) = &mut self.attempt_cleanup else {
            return Ok(false);
        };
        if now < attempt.until {
            return Ok(false);
        }
        self.requests.abandon(attempt.index)?;
        attempt.failed.get_or_insert(AttemptCleanupFailure::Deadline);
        Ok(true)
    }

    pub(super) fn advance_attempt_cleanup(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let now = resources.execution.client.now()?;
        let Some(index) = self.observe_attempt_retirement(now, |lease| {
            let cleanup_failed = lease
                .is_some_and(|lease| resources.backend.cleanup_failure(lease.resource).is_some());
            let released = if cleanup_failed {
                false
            } else {
                match lease {
                    Some(lease) => resources.backend.attempt_released(lease)?,
                    None => true,
                }
            };
            Ok(BackendRetirement { cleanup_failed, released })
        })?
        else {
            return Ok(());
        };
        let attempt = self.attempt_cleanup.as_ref().ok_or(ParentError::WrongPhase)?;
        if let Some(resource) = attempt.resource
            && resources.backend.failure(resource).is_some()
        {
            let origin = attempt.origin;
            let identity = attempt.identity;
            let campaign_scope = resources.backend.campaign_scope(resource);
            let slot = self
                .backend_causes
                .as_mut()
                .ok_or(ParentError::WrongPhase)?
                .free_slot()
                .ok_or(ParentError::Overloaded)?;
            if let Some(cause) = resources.backend.take_attempt_failure(resource)? {
                *slot = Some(SettledAttemptFailure { identity, resource, cause });
                tracing::info!(operation="startup_attempt_settled",request=?origin.request,
                    identity=?identity,resource=?resource,campaign_scope=?campaign_scope,
                    "Original preparation cause retained after checked settlement");
            }
        }
        self.stage_settlement_reply(index)
    }

    /// Retire old semantic correlations only after the original actual settlement
    /// ACK fully wrote. Ready metadata, cancellation or a queued ACK is insufficient.
    pub(super) fn finish_attempt_write(&mut self, header: WireHeader, now: Moment) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        let Some(attempt) = &self.attempt_cleanup else {
            return Ok(());
        };
        if self.requests.get(attempt.index)?.header != header {
            return Ok(());
        }
        if self.expire_attempt(now)? {
            return Ok(());
        }
        let attempt = self.attempt_cleanup.as_ref().ok_or(ParentError::WrongPhase)?;
        if !attempt.ready || attempt.failed.is_some() || !self.requests.get(attempt.index)?.sent {
            return Err(ParentError::WrongPhase.into());
        }
        self.requests.settle_abandoned(attempt.origin.request.ok_or(ParentError::WrongPhase)?);
        self.acquisition = None;
        self.lease = None;
        self.recovery = super::recovery::Recovery::default();
        self.reset_output();
        self.backend_failure_seen = false;
        self.backend_failed_resource = None;
        self.attempt_cleanup = None;
        Ok(())
    }
}

/// Translate the fixed original fifteen-second interval from its actual decoded QPC cutoff.
/// Waiting for native main retirement cannot renew backend cleanup.
fn attempt_budget(until: Moment, now: Moment) -> std::result::Result<ShutdownBudget, ParentError> {
    let started = until.as_nanos().checked_sub(15_000_000_000).ok_or(ParentError::WrongPhase)?;
    let started = Moment::from_nanos(started);
    if now < started || now >= until {
        return Err(ParentError::WrongPhase);
    }
    let original = ShutdownBudget::new(started).map_err(|_| ParentError::WrongPhase)?;
    if original.settle_until != until {
        return Err(ParentError::WrongPhase);
    }
    Ok(original)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_attempt_request_keeps_original_fifteen_second_cleanup_interval()
    -> std::result::Result<(), ParentError> {
        let until = Moment::from_nanos(15_000_000_000);
        let early = attempt_budget(until, Moment::from_nanos(1))?;
        let delayed = attempt_budget(until, Moment::from_nanos(13_000_000_000))?;
        assert_eq!(early, delayed);
        assert_eq!(delayed.started, Moment::from_nanos(0));
        assert_eq!(delayed.settle_until, until);
        assert!(attempt_budget(until, until).is_err());
        assert!(attempt_budget(Moment::from_nanos(16_000_000_000), Moment::from_nanos(0)).is_err());
        Ok(())
    }
}
