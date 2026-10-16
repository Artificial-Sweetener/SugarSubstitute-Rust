// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the mounted Main creation identity immutable while its authenticated backend changes.
use super::*;
use sugarsubstitute_contracts::lifecycle::LeaseKey;
use sugarsubstitute_desktop::MainBackendState;

#[derive(Clone, Copy)]
pub(super) struct Outage {
    failed: LeaseReference,
    ready: bool,
    displayed: MainBackendState,
}
/// A recognized obsolete native callback has no wire, binding, source or window effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetryRequestDisposition {
    Queued,
    Obsolete,
}
/// Data binding only: Parent separately authenticates retained Ready source/origin/version proof.
/// Neither this type nor presentation state grants a browser or managed process capability.
pub(super) struct RunningBackendBinding {
    identity: MainIdentity,
    connection: ConnectionId,
    current: LeaseReference,
    previous: Option<LeaseReference>,
    outage: Option<Outage>,
    retry: Option<ConnectionBudget>,
    retry_expired: bool,
    awaiting_positive: Option<LeaseReference>,
}
impl RunningBackendBinding {
    pub(super) fn new(
        identity: MainIdentity,
        binding: crate::production_output::OutputBinding,
    ) -> Result<Self> {
        if binding.run != identity.run || binding.resource.owner != identity.run.owner {
            return Err("Mounted backend binding differs from current Main".into());
        }
        Ok(Self {
            identity,
            connection: binding.connection,
            current: binding.lease,
            previous: None,
            outage: None,
            retry: None,
            retry_expired: false,
            awaiting_positive: None,
        })
    }
    pub(super) fn key(&self, reference: LeaseReference) -> LeaseKey {
        LeaseKey {
            resource: ResourceKey {
                owner: self.identity.run.owner,
                id: reference.resource,
                generation: reference.generation,
            },
            id: reference.lease,
            run: self.identity.run,
            connection: self.connection,
        }
    }
    /// Coalesce repeated notices without changing the original failed identity or retry intent.
    pub(super) fn unavailable(
        &mut self,
        failed: LeaseReference,
        ready: bool,
    ) -> Result<Option<(MainBackendState, Option<LeaseKey>)>> {
        if failed != self.current {
            return Err("Unavailable backend is not the current mounted binding".into());
        }
        let state = if self.retry_expired {
            MainBackendState::Retiring
        } else if self.retry.is_some() {
            MainBackendState::Retrying
        } else if ready {
            MainBackendState::RetryReady
        } else {
            MainBackendState::Retiring
        };
        let changed = self.outage.is_none_or(|old| old.failed != failed || old.displayed != state);
        let previous =
            self.outage.filter(|old| old.failed != failed).map(|old| self.key(old.failed));
        self.awaiting_positive = None;
        self.outage =
            Some(Outage { failed, ready: ready && !self.retry_expired, displayed: state });
        Ok(changed.then_some((state, previous)))
    }
    /// Consume one real callback; only known same-Main outage/handoff races are benign.
    pub(super) fn retry(
        &mut self,
        identity: MainIdentity,
        failed: LeaseKey,
        now: Moment,
    ) -> Result<RetryRequestDisposition> {
        if identity != self.identity {
            return Err("Retry callback belongs to another Main".into());
        }
        let old = self.previous.is_some_and(|previous| failed == self.key(previous));
        let displayed = self.outage.filter(|outage| failed == self.key(outage.failed));
        if old && failed != self.key(self.current) {
            return Ok(RetryRequestDisposition::Obsolete);
        }
        let outage = displayed.ok_or("Retry callback has no actual known outage or handoff")?;
        if failed != self.key(self.current)
            || !outage.ready
            || self.retry.is_some()
            || self.awaiting_positive.is_some()
            || self.retry_expired
        {
            return Ok(RetryRequestDisposition::Obsolete);
        }
        let budget = ConnectionBudget::new(
            sugarsubstitute_application::startup::ConnectionBudgetKind::Managed,
            now,
        )?;
        if let Some(outage) = &mut self.outage {
            outage.displayed = MainBackendState::Retrying;
        }
        self.retry_expired = false;
        self.retry = Some(budget);
        Ok(RetryRequestDisposition::Queued)
    }
    pub(super) fn retry_deadline(&self) -> Option<Moment> {
        self.retry.filter(|_| !self.retry_expired).map(|budget| budget.until())
    }
    pub(super) fn expire_retry(&mut self, now: Moment) -> Option<LeaseKey> {
        if self.retry_expired || self.retry.is_none_or(|budget| now < budget.until()) {
            return None;
        }
        self.retry_expired = true;
        if let Some(outage) = &mut self.outage {
            outage.ready = false;
            outage.displayed = MainBackendState::Retiring;
        }
        Some(self.key(self.current))
    }
    pub(super) fn retry_request(&self) -> Option<(MainIdentity, LeaseReference, Moment)> {
        self.retry
            .filter(|_| !self.retry_expired)
            .map(|budget| (self.identity, self.current, budget.until()))
    }
    pub(super) fn validate_replacement(
        &self,
        identity: MainIdentity,
        previous: LeaseReference,
        current: LeaseReference,
    ) -> Result<()> {
        if identity != self.identity
            || previous != self.current
            || previous == current
            || (previous.resource == current.resource && previous.generation == current.generation)
        {
            return Err("Replacement differs from the current mounted Main/backend pair".into());
        }
        Ok(())
    }
    pub(super) fn replacing(
        &mut self,
        identity: MainIdentity,
        previous: LeaseReference,
        current: LeaseReference,
    ) -> Result<LeaseKey> {
        self.validate_replacement(identity, previous, current)?;
        self.outage =
            Some(Outage { failed: previous, ready: false, displayed: MainBackendState::Retrying });
        Ok(self.key(previous))
    }
    /// Adopt only after actual new output-source admission and old frame/request retirement.
    /// Historical Ready is binding provenance; positive current output separately clears UI outage.
    pub(super) fn replaced(
        &mut self,
        identity: MainIdentity,
        previous: LeaseReference,
        current: LeaseReference,
    ) -> Result<()> {
        self.validate_replacement(identity, previous, current)?;
        self.awaiting_positive = self.outage.map(|outage| outage.failed);
        self.previous = Some(previous);
        self.current = current;
        self.retry = None;
        self.retry_expired = false;
        Ok(())
    }
    /// The worker has consumed an authenticated ordinary reply for the new current binding.
    pub(super) fn positive(&mut self, current: LeaseReference) -> Result<Option<LeaseKey>> {
        if current != self.current {
            return Err("Ordinary output is not the mounted current binding".into());
        }
        let cleared = self.awaiting_positive.take().map(|old| self.key(old));
        if cleared.is_some() {
            self.outage = None;
        }
        Ok(cleared)
    }
    pub(super) fn retry_failed(&mut self) -> Result<LeaseKey> {
        if self.retry.take().is_none() {
            return Err("No original retry owns this disposition".into());
        }
        let outage = self.outage.as_mut().ok_or("Original retry outage absent")?;
        outage.ready = false;
        outage.displayed = MainBackendState::Retiring;
        Ok(self.key(self.current))
    }
}
impl ProductionChild {
    pub(super) fn ensure_running_backend(&mut self) -> Result<()> {
        if self.phase != Some(sugarsubstitute_application::startup::StartupPhase::Running)
            || self.shutdown.closing()
            || self.main.profile().is_none_or(|profile| !profile.authorized())
        {
            return Ok(());
        }
        if self.running_backend.is_none()
            && let Some(binding) = self.output.binding
        {
            if self.lease != Some(binding.lease) {
                return Err("Mounted current lease differs from original grant".into());
            }
            self.running_backend = Some(RunningBackendBinding::new(self.identity, binding)?);
        }
        if self.running_backend.is_some()
            && let Some(owner) = self.output.owner_mut()
        {
            owner.running_main(self.identity)?;
        }
        Ok(())
    }
    pub(super) fn backend_retry_requested(
        &mut self,
        identity: MainIdentity,
        failed: LeaseKey,
    ) -> Result<()> {
        self.ensure_running_backend()?;
        if self.shutdown.closing() || self.output.closing() {
            return Err("Retry arrived after actual child close".into());
        }
        let now = self.client()?.now()?;
        let disposition = self
            .running_backend
            .as_mut()
            .ok_or("Current mounted backend absent")?
            .retry(identity, failed, now)?;
        if disposition == RetryRequestDisposition::Obsolete {
            tracing::debug!(operation="mounted_backend_retry",identity=?identity,failed=?failed,
                disposition="obsolete", "Consumed known obsolete native retry callback");
            return Ok(());
        }
        self.output.retry_expired = false;
        self.output.pull_after = None;
        self.push_command(DesktopStartupCommand::PresentBackendOutage {
            identity,
            failed,
            state: MainBackendState::Retrying,
        })
    }
}

#[cfg(test)]
#[path = "output/recovery_tests.rs"]
mod tests;
