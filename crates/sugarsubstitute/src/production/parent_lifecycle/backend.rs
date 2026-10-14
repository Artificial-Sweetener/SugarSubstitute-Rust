// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve each original Acquire until actual backend readiness publishes one final reply.
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sugarsubstitute_contracts::lifecycle::{
    ClientBinding, LifecycleFailureCode, RequestOutcome, ResourceRequest, ResourceTransition,
};

impl ParentLifecycle {
    pub(super) fn backend_request(
        &mut self,
        binding: ClientBinding,
        request: ResourceRequest,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let original_request = request.id;
        let original_budget =
            if let sugarsubstitute_contracts::lifecycle::ResourceCommand::Acquire(target) =
                &request.command
            {
                let index =
                    self.requests.find(original_request).ok_or(ParentError::StaleRequest)?;
                let until = self.requests.get(index)?.until.ok_or(ParentError::WrongPhase)?;
                let kind = match target {
                    sugarsubstitute_contracts::lifecycle::BackendTarget::Managed(_) => {
                        sugarsubstitute_application::startup::ConnectionBudgetKind::Managed
                    }
                    sugarsubstitute_contracts::lifecycle::BackendTarget::Attached(_) => {
                        sugarsubstitute_application::startup::ConnectionBudgetKind::Attached
                    }
                };
                Some(sugarsubstitute_application::startup::ConnectionBudget::from_until(
                    kind,
                    resources.execution.client.now()?,
                    until,
                )?)
            } else {
                None
            };
        match resources.backend.request(
            binding,
            request,
            original_budget,
            resources.preparation,
            resources.execution,
        ) {
            Ok(transition) => self.backend_replies(transition, resources),
            Err(cause) => {
                self.forwarded = resources.backend.take_pending_transition();
                let observed = self
                    .observe_pending_resources()
                    .and_then(|()| self.latch_backend_failure(original_request));
                if let Err(additional) = observed {
                    self.retain_failure(cause);
                    return Err(additional.into());
                }
                // A typed denial with no observed resource admitted no native
                // preparation. Keep the original cause and send its bounded code.
                if self.requests.backend_resource(original_request)?.is_none()
                    && let Some(code) = cause
                        .downcast_ref::<sugarsubstitute_contracts::lifecycle::LifecycleRejection>()
                        .copied()
                {
                    let index =
                        self.requests.find(original_request).ok_or(ParentError::StaleRequest)?;
                    self.requests.get_mut(index)?.reply = Some(Reply::Rejected(code));
                    self._attempt_failure = Some(cause);
                    self.forwarded = None;
                    return Ok(());
                }
                Err(cause)
            }
        }
    }

    fn observe_pending_resources(&mut self) -> std::result::Result<(), ParentError> {
        let Some(transition) = &self.forwarded else {
            return Ok(());
        };
        for reply in &transition.replies {
            if reply.run == self.run && matches!(reply.outcome, RequestOutcome::Pending(_)) {
                let index = self.requests.find(reply.request).ok_or(ParentError::StaleRequest)?;
                let _fresh = self.requests.observe_backend(index, reply.outcome)?;
            }
        }
        Ok(())
    }

    fn latch_backend_failure(
        &mut self,
        request: sugarsubstitute_contracts::lifecycle::RequestId,
    ) -> std::result::Result<(), ParentError> {
        let actual = self.requests.backend_resource(request)?;
        if !self.backend_failure_seen {
            self.backend_failed_resource = actual;
            self.backend_failure_seen = true;
        }
        Ok(())
    }

    /// Route transitions once after the coordinator consumed its own actual jobs.
    /// Pending remains internal; it is never transmitted as a completed request.
    pub(crate) fn backend_transition(
        &mut self,
        transition: &ResourceTransition,
        resources: &mut ParentResources<'_, '_>,
    ) -> std::result::Result<(), ParentError> {
        for reply in &transition.replies {
            if let Err(cause) = self.backend_reply(*reply, resources) {
                self.retain_failure(cause);
                return Err(ParentError::OwnerFailed);
            }
        }
        Ok(())
    }

    pub(super) fn backend_replies(
        &mut self,
        transition: ResourceTransition,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        if self.forwarded.is_some() {
            return Err(ParentError::Overloaded.into());
        }
        // Adopt the complete producer result before any local identity/proof check.
        self.forwarded = Some(transition);
        let count = self.forwarded.as_ref().ok_or(ParentError::WrongPhase)?.replies.len();
        for index in 0..count {
            let reply = self.forwarded.as_ref().ok_or(ParentError::WrongPhase)?.replies[index];
            self.backend_reply(reply, resources)?;
        }
        let transition = self.forwarded.as_mut().ok_or(ParentError::WrongPhase)?;
        transition.replies.retain(|reply| reply.run != self.run);
        transition.effects.clear();
        if transition.replies.is_empty() {
            self.forwarded = None;
        }
        Ok(())
    }

    fn backend_reply(
        &mut self,
        reply: sugarsubstitute_contracts::lifecycle::ResourceReply,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        if reply.run != self.run {
            return Ok(());
        }
        let index = self.requests.find(reply.request).ok_or(ParentError::StaleRequest)?;
        if !self.requests.observe_backend(index, reply.outcome)? {
            return Ok(());
        }
        let record = self.requests.get(index)?;
        let final_reply = match reply.outcome {
            RequestOutcome::Pending(_) => return Ok(()),
            RequestOutcome::Granted(lease) => {
                if self.closed {
                    return Err(ParentError::WrongPhase.into());
                }
                resources.session.control.validate_current_admission()?;
                let identity = record.identity.ok_or(ParentError::WrongPhase)?;
                let until = record.until.ok_or(ParentError::WrongPhase)?;
                if resources.execution.client.now()? >= until {
                    return Err(ParentError::WrongPhase.into());
                }
                resources
                    .backend
                    .readiness(lease.resource)
                    .ok_or(ParentError::WrongPhase)?
                    .validate_current()?;
                if resources.backend.ready_origin(lease.resource)?.is_none() {
                    return Err(ParentError::WrongPhase.into());
                }
                let frontend = sugarsubstitute_application::startup::FrontendBudget::new(
                    resources.execution.client.now()?,
                )?;
                self.main.begin(identity, lease, frontend.until())?;
                resources.profile.begin_main_attempt(identity)?;
                self.folders.begin(identity);
                self.lease = Some(lease);
                Reply::Granted { lease, identity }
            }
            RequestOutcome::Released => {
                self.lease = None;
                self.output = None;
                Reply::Released
            }
            RequestOutcome::Reloaded(lease) => Reply::Reloaded(lease),
            RequestOutcome::Cancelled => Reply::Cancelled,
            RequestOutcome::Failed => {
                self.latch_backend_failure(reply.request)?;
                Reply::Failed(LifecycleFailureCode::Preparation)
            }
        };
        tracing::info!(operation="parent_backend_disposition",request=?reply.request,run=?self.run,
            disposition=?reply.outcome,main_stage=?self.main.stage(),
            "Original final backend disposition retained by parent");
        self.requests.get_mut(index)?.reply = Some(final_reply);
        Ok(())
    }
}
