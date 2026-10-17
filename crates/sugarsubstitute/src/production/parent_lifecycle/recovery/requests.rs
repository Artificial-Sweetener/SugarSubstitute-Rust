// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit one actual native Retry against the retained exact managed outage and original cause slot.
use super::*;
use sugarsubstitute_application::{
    startup::{ConnectionBudget, ConnectionBudgetKind},
    supervision::InvalidatedLeaseState,
};
use sugarsubstitute_contracts::lifecycle::{BackendTarget, ResourceCommand, ResourceRequest};
use sugarsubstitute_supervision::lifecycle_protocol::LeaseReference;
impl ParentLifecycle {
    /// Move the sole original settled cause into an existing funded owner before exposing RetryReady.
    /// Occupied slots and failed physical settlement leave both cause owners unchanged.
    pub(in crate::production::parent_lifecycle) fn recovery_retry_ready(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<bool> {
        let Some(outage) = self.recovery.outage else {
            return Ok(false);
        };
        let binding = self.binding()?;
        // Another cohort member may already own the sole original transferred cause.
        // Authoritative App RetryReady includes that actual transfer; never demand a second copy.
        if resources.backend.invalidated_lease(binding, outage.failed)?
            == InvalidatedLeaseState::RetryReady
        {
            return Ok(true);
        }
        let key = resources.backend.recovery_cause_key(binding, outage.failed)?;
        if self.backend_causes.as_ref().is_some_and(|owner| owner.contains(key)) {
            return Ok(resources.backend.invalidated_lease(binding, outage.failed)?
                == InvalidatedLeaseState::RetryReady);
        }
        let origin = self.acquisition.as_ref().ok_or(ParentError::WrongPhase)?.header;
        let identity = self.main.snapshot().identity.ok_or(ParentError::WrongPhase)?;
        let campaign_scope = resources.backend.campaign_scope(key);
        let Some(slot) = self.backend_causes.as_mut().ok_or(ParentError::WrongPhase)?.free_slot()
        else {
            return Ok(false);
        };
        let mut cause = None;
        if !resources.backend.transfer_recovery_failure(key, &mut cause)? {
            return Ok(false);
        }
        // The backend's success contract guarantees Some; retaining the returned slot still precedes any later check.
        if let Some(cause) = cause {
            *slot = Some(super::super::attempt::SettledAttemptFailure {
                identity,
                resource: key,
                cause,
            });
        } else {
            return Err(ParentError::WrongPhase.into());
        }
        tracing::info!(operation="native_backend_failure_retained",request=?origin.request,identity=?identity,resource=?key,campaign_scope=?campaign_scope,
            "Original settled backend cause retained in its existing native owner");
        Ok(resources.backend.invalidated_lease(binding, outage.failed)?
            == InvalidatedLeaseState::RetryReady)
    }
    /// Preserve this one click's exact original header/cutoff; no browser/profile/Main startup is invoked.
    pub(in crate::production::parent_lifecycle) fn retry_backend(
        &mut self,
        index: usize,
        identity: MainIdentity,
        failed: LeaseReference,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        Recovery::main_identity(self, identity)?;
        let outage = self.recovery.outage.ok_or(ParentError::WrongPhase)?;
        if super::super::replies::reference(outage.failed) != failed
            || !matches!(resources.target, BackendTarget::Managed(_))
            || self.output.is_some()
            || self.recovery.handoff.is_some()
            || self.recovery.retry.is_some()
        {
            return Err(ParentError::WrongPhase.into());
        }
        let header = self.requests.get(index)?.header;
        let until = self.requests.get(index)?.until.ok_or(ParentError::WrongPhase)?;
        ConnectionBudget::from_until(
            ConnectionBudgetKind::Managed,
            resources.execution.client.now()?,
            until,
        )?;
        if !self.recovery_retry_ready(resources)? {
            return Err(ParentError::WrongPhase.into());
        }
        self.requests.get_mut(index)?.identity = Some(identity);
        self.recovery.retry = Some(Retry {
            header,
            failed: outage.failed,
            until,
            successor: None,
            outcome: None,
            expired: false,
        });
        self.backend_request(
            self.binding()?,
            ResourceRequest {
                run: self.run,
                connection: self.connection,
                id: header.request.ok_or(ParentError::WrongPhase)?,
                command: ResourceCommand::RetryDead(outage.failed),
            },
            resources,
        )
    }
    /// Retain actual full native completion only; an issued or queued replacement cannot acknowledge adoption.
    pub(in crate::production::parent_lifecycle) fn recovery_written(
        &mut self,
        index: usize,
    ) -> Result<()> {
        let record = self.requests.get(index)?;
        match record.reply {
            Some(Reply::BackendReplaced { pair, .. }) => {
                self.recovery.written(record.header)?;
                tracing::info!(operation="native_backend_binding_written",request=?record.header.request,previous=?pair.previous,current=?pair.current,
                    "Actual full native replacement write retained; child binding consumption still pending");
            }
            Some(Reply::Failed(_) | Reply::Rejected(_) | Reply::Cancelled)
                if self.recovery.retry.is_some_and(|retry| retry.header == record.header) =>
            {
                self.recovery.retry = None
            }
            _ => {}
        }
        Ok(())
    }
}

impl ParentLifecycle {
    /// Requester progress/final outcome never re-enters initial Main/Profile authorization.
    pub(in crate::production::parent_lifecycle) fn recovery_backend_reply(
        &mut self,
        reply: sugarsubstitute_contracts::lifecycle::ResourceReply,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        use sugarsubstitute_contracts::lifecycle::{LifecycleFailureCode, RequestOutcome};
        let retry = self.recovery.retry.ok_or(ParentError::WrongPhase)?;
        let index = self.requests.find(reply.request).ok_or(ParentError::StaleRequest)?;
        if reply.run != self.run || retry.header != self.requests.get(index)?.header {
            return Err(ParentError::ForeignPeer.into());
        }
        if let RequestOutcome::Granted(current) = reply.outcome {
            let handoff = self.recovery.handoff.ok_or(ParentError::WrongPhase)?;
            if handoff.pair.previous != retry.failed
                || handoff.pair.current != current
                || self.output.is_some()
            {
                return Err(ParentError::ForeignPeer.into());
            }
        }
        let final_outcome = self.requests.observe_backend(index, reply.outcome)?;
        if !matches!(reply.outcome, RequestOutcome::Pending(_)) {
            self.recovery.retry.as_mut().ok_or(ParentError::WrongPhase)?.outcome =
                Some(reply.outcome);
        }
        self.expire_recovery(resources.execution.client.now()?)?;
        if self.recovery.retry.is_none_or(|retry| retry.expired) {
            return Ok(());
        }
        match reply.outcome {
            RequestOutcome::Pending(lease) => {
                self.recovery.retry.as_mut().ok_or(ParentError::WrongPhase)?.successor =
                    Some(lease);
            }
            RequestOutcome::Granted(_) if final_outcome => self.stage_replacement(index)?,
            RequestOutcome::Failed if final_outcome => {
                self.requests.get_mut(index)?.reply =
                    Some(Reply::Failed(LifecycleFailureCode::Preparation))
            }
            RequestOutcome::Cancelled if final_outcome => {
                self.requests.get_mut(index)?.reply = Some(Reply::Cancelled)
            }
            RequestOutcome::Granted(_) | RequestOutcome::Failed | RequestOutcome::Cancelled => {}
            _ => return Err(ParentError::WrongPhase.into()),
        }
        Ok(())
    }
}

impl ParentLifecycle {
    /// Retain one authenticated original command in its already-funded request record.
    pub(in crate::production::parent_lifecycle) fn hold_backend_command(
        &mut self,
        index: usize,
        command: super::super::history::HeldBackendCommand,
    ) -> Result<()> {
        if self.requests.held_backend().is_some() {
            return Err(ParentError::Overloaded.into());
        }
        self.requests.get_mut(index)?.held_backend = Some(command);
        Ok(())
    }
    /// Admit retained Release during accepted exit intent; a physically closing channel cancels all work.
    /// Original request deadlines and pending producer delivery remain binding during role cleanup.
    pub(in crate::production::parent_lifecycle) fn next_backend_command(
        &mut self,
        now: sugarsubstitute_contracts::execution::Moment,
        backend_delivery_pending: bool,
        backend_commands_open: bool,
    ) -> Result<Option<(usize, super::super::history::HeldBackendCommand)>> {
        use super::super::history::HeldBackendCommand;
        let Some(index) = self.requests.held_backend() else {
            return Ok(None);
        };
        let command = self.requests.get(index)?.held_backend.ok_or(ParentError::WrongPhase)?;
        let release = matches!(command, HeldBackendCommand::Release(_));
        if self.closed || (self.cleanup.is_some() && !release) {
            self.requests.cancel_held(index)?;
            self.retain_held_terminal(index)?;
            return Ok(None);
        }
        let cleanup_command = release || matches!(command, HeldBackendCommand::Reload { .. });
        let open = self.forwarded.is_none()
            && !backend_delivery_pending
            && (cleanup_command || backend_commands_open);
        let command = self.requests.poll_held_backend(now, open)?;
        self.retain_held_terminal(index)?;
        Ok(command)
    }
    /// Preserve a genuine never-admitted terminal for the configured managed semantic waiter.
    /// This admits no backend effect and supplies no native settlement witness.
    fn retain_held_terminal(&mut self, index: usize) -> Result<()> {
        let record = self.requests.get(index)?;
        if self.requests.managed_waiting(index)?
            && record.held_terminal.is_some()
            && let Some(acquisition) = &mut self.acquisition
            && acquisition.header == record.header
        {
            acquisition.reply(record.header, record.reply.ok_or(ParentError::WrongPhase)?)?;
        }
        Ok(())
    }
    /// Retry only the original command after Root's shared gate opens, never its deadline or an admitted effect.
    pub(in crate::production::parent_lifecycle) fn advance_backend_commands(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        use super::super::history::HeldBackendCommand;
        use sugarsubstitute_contracts::lifecycle::LifecycleRejection;
        let Some((index, command)) = self.next_backend_command(
            resources.execution.client.now()?,
            resources.backend.delivery_pending(),
            resources.backend_commands_open,
        )?
        else {
            return Ok(());
        };
        let header = self.requests.get(index)?.header;
        if self.requests.get(index)?.until.is_none() {
            return Err(ParentError::WrongPhase.into());
        }
        match command {
            HeldBackendCommand::Acquire(identity) => {
                if self.acquisition.as_ref().is_none_or(|acquisition| acquisition.header != header)
                    || self.cleanup.is_some()
                    || self.attempt_cleanup.is_some()
                    || self.main.stage()
                        != sugarsubstitute_application::supervision::MainAuthorizationStage::Retired
                {
                    return Err(ParentError::WrongPhase.into());
                }
                let request = header.request.ok_or(ParentError::WrongPhase)?;
                self.requests.get_mut(index)?.identity = Some(identity);
                self.backend_request(
                    self.binding()?,
                    ResourceRequest {
                        run: self.run,
                        connection: self.connection,
                        id: request,
                        command: ResourceCommand::Acquire(resources.target.clone()),
                    },
                    resources,
                )
            }
            HeldBackendCommand::Release(lease) => self.backend_request(
                self.binding()?,
                ResourceRequest {
                    run: self.run,
                    connection: self.connection,
                    id: header.request.ok_or(ParentError::WrongPhase)?,
                    command: ResourceCommand::Release(lease),
                },
                resources,
            ),
            HeldBackendCommand::Reload { identity, lease } => {
                self.native_reload(index, identity, lease, resources)
            }
            HeldBackendCommand::Retry { identity, failed } => {
                if self
                    .recovery
                    .outage
                    .is_none_or(|outage| super::super::replies::reference(outage.failed) != failed)
                    || self.recovery.handoff.is_some()
                    || self.recovery.retry.is_some()
                {
                    self.requests.get_mut(index)?.reply =
                        Some(Reply::Rejected(LifecycleRejection::WrongPhase));
                    return Ok(());
                }
                if !self.recovery_retry_ready(resources)? {
                    self.requests.get_mut(index)?.reply =
                        Some(Reply::Rejected(LifecycleRejection::WrongPhase));
                    return Ok(());
                }
                self.retry_backend(index, identity, failed, resources)
            }
        }
    }
}

impl ParentLifecycle {
    /// Retain the original campaign cutoff and actual write custody on expected retry expiry.
    /// A final producer outcome and absence of its native writer retire only the logical request.
    pub(in crate::production::parent_lifecycle) fn expire_recovery(
        &mut self,
        now: Moment,
    ) -> Result<()> {
        let Some(retry) = self.recovery.retry else {
            return Ok(());
        };
        if self.requests.managed_waiting(
            self.requests
                .find(retry.header.request.ok_or(ParentError::WrongPhase)?)
                .ok_or(ParentError::StaleRequest)?,
        )? || now < retry.until
        {
            return Ok(());
        }
        let index = self
            .requests
            .find(retry.header.request.ok_or(ParentError::WrongPhase)?)
            .ok_or(ParentError::StaleRequest)?;
        if self.requests.get(index)?.sent {
            return Ok(());
        }
        self.recovery.retry.as_mut().ok_or(ParentError::WrongPhase)?.expired = true;
        self.requests.timeout_backend(index)?;
        let pending_write = self.write.as_ref().is_some_and(|write| matches!(write.purpose, super::super::types::WritePurpose::Reply(at) if at == index));
        if pending_write {
            if let Some(write) = &self.write {
                let _disposition = write.admission.revoke();
            }
        } else if retry.outcome.is_some() {
            self.requests.retire_timed_out(index)?;
            self.recovery.retry = None;
        }
        Ok(())
    }
}
