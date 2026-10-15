// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep old raw-output consumption before replacement and authenticate new binding consumption.
use super::*;
use sugarsubstitute_contracts::lifecycle::LifecycleRejection;
use sugarsubstitute_supervision::lifecycle_protocol::LeaseReference;
impl ParentLifecycle {
    /// Return a binding only after authentic current-child observation and actual App/live-or-dead validation.
    /// Source selection was captured at recipient acceptance and is checked again before ACK acceptance.
    pub(in crate::production::parent_lifecycle) fn running_output_lease(
        &mut self,
        reference: LeaseReference,
        acknowledged: Option<std::num::NonZeroU64>,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<Option<LeaseKey>> {
        let Some(running) = self.recovery.running else {
            return Ok(None);
        };
        if let Some(handoff) = self.recovery.handoff
            && super::super::replies::reference(handoff.pair.current) == reference
        {
            if self.output.is_some() {
                return Err(ParentError::WrongPhase.into());
            }
            let binding = self.binding()?;
            let mut next = self.recovery;
            next.consume_binding(handoff.pair.current, acknowledged, |pair, invalidation| {
                if super::replacement_document(binding, pair, resources)? != running.document {
                    return Err(ParentError::ProfileMismatch.into());
                }
                if let Some(notice) = invalidation {
                    if notice.lease != pair.current {
                        return Err(ParentError::ForeignPeer.into());
                    }
                    // Independently authenticates same Run/connection and exact actual new outage first.
                    resources.backend.invalidated_lease(binding, pair.current)?;
                    match resources.backend.invalidated_lease(binding, pair.previous) {
                        Err(cause)
                            if cause.downcast_ref::<LifecycleRejection>()
                                == Some(&LifecycleRejection::StaleIdentity) =>
                        {
                            Ok(())
                        }
                        Err(cause) => Err(cause),
                        Ok(_) => Err(ParentError::WrongPhase.into()),
                    }
                } else {
                    resources
                        .backend
                        .readiness(pair.current.resource)
                        .ok_or(ParentError::WrongPhase)?
                        .validate_current()?;
                    if resources.backend.ready_origin(pair.current.resource)?
                        != Some(running.document.origin)
                    {
                        return Err(ParentError::ProfileMismatch.into());
                    }
                    resources.backend.adopt_replacement(binding, pair.previous, pair.current)
                }
            })?;
            // All fallible local/producer checks preceded the irreversible live App adoption.
            self.recovery = next;
            tracing::info!(operation="native_backend_binding_consumed",previous=?handoff.pair.previous,current=?handoff.pair.current,
                current_unavailable=self.recovery.outage.is_some(),
                "Authenticated current pull consumed the original native binding handoff");
            self.reset_output();
            self.output_lease = Some(handoff.pair.current);
            return Ok(Some(handoff.pair.current));
        }
        if super::super::replies::reference(running.lease) != reference {
            return Err(ParentError::ForeignPeer.into());
        }
        Ok(Some(running.lease))
    }
    /// Answer an actual old pull immediately; a pending explicit Retry owns its own final header.
    /// Exact old ACK validation/disposal has already run through the real output owner.
    pub(in crate::production::parent_lifecycle) fn recovery_output_reply(
        &mut self,
        index: usize,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<bool> {
        let Some(outage) = self.recovery.outage else {
            return Ok(false);
        };
        if self.output.is_some() {
            return Err(ParentError::WrongPhase.into());
        }
        if self.recovery.handoff.is_some() && self.recovery.retry.is_none() {
            self.stage_replacement(index)?;
            return Ok(true);
        }
        let affected_clients =
            resources.backend.affected_clients(self.binding()?, outage.failed)?;
        let retry_ready = self.recovery.retry.is_none() && self.recovery_retry_ready(resources)?;
        self.requests.get_mut(index)?.reply = Some(Reply::BackendUnavailable {
            failed: outage.failed,
            status: outage.status,
            affected_clients,
            retry_ready,
        });
        Ok(true)
    }
    /// One actual header consumes the replacement response; this never starts/retires Main or a profile.
    pub(in crate::production::parent_lifecycle) fn stage_replacement(
        &mut self,
        index: usize,
    ) -> Result<()> {
        if self.output.is_some() {
            return Err(ParentError::WrongPhase.into());
        }
        let identity = self.main.snapshot().identity.ok_or(ParentError::WrongPhase)?;
        Recovery::main_identity(self, identity)?;
        let header = self.requests.get(index)?.header;
        let pair = self.recovery.stage(header)?;
        self.requests.get_mut(index)?.reply = Some(Reply::BackendReplaced { identity, pair });
        tracing::info!(operation="native_backend_binding_staged",request=?header.request,identity=?identity,previous=?pair.previous,current=?pair.current,
            "Original native request retained one binding-only replacement reply");
        Ok(())
    }
}

impl ParentLifecycle {
    /// Require a still-live native proof or the separately accepted exact successor death notice before issue.
    pub(in crate::production::parent_lifecycle) fn replacement_reply_ready(
        &self,
        pair: GrantedResourceReplacement,
        resources: &ParentResources<'_, '_>,
    ) -> Result<bool> {
        let handoff = self.recovery.handoff.ok_or(ParentError::WrongPhase)?;
        if handoff.pair != pair {
            return Err(ParentError::ForeignPeer.into());
        }
        // A real B death may precede delivery of its typed notice to this recipient.
        // Keep the original reply pending until that exact notice is separately accepted.
        if handoff.next.is_none() {
            match resources.backend.invalidated_lease(self.binding()?, pair.current) {
                Ok(_) => return Ok(false),
                Err(cause)
                    if cause.downcast_ref::<LifecycleRejection>()
                        == Some(&LifecycleRejection::StaleIdentity) => {}
                Err(cause) => return Err(cause),
            }
        }
        if super::replacement_document(self.binding()?, pair, resources)?
            != self.recovery.running.ok_or(ParentError::WrongPhase)?.document
        {
            return Err(ParentError::ProfileMismatch.into());
        }
        if let Some(readiness) = resources.backend.readiness(pair.current.resource) {
            readiness.validate_current()?;
            return Ok(true);
        }
        let Some(notice) = handoff.next else {
            return Ok(false);
        };
        if notice.lease != pair.current {
            return Err(ParentError::ForeignPeer.into());
        }
        resources.backend.invalidated_lease(self.binding()?, pair.current)?;
        Ok(true)
    }
}
