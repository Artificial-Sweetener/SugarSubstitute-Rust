// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve a known outage cause before actual App disconnect retires its last authenticated interest.
use super::*;
impl ParentLifecycle {
    /// Preserve a known outage's sole original cause before disconnect can remove its App lineage.
    /// The retained authenticated registration is usable for cleanup queries after ordinary admission closes.
    pub(in crate::production::parent_lifecycle) fn recovery_close_cause(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<bool> {
        let Some(outage) = self.recovery.outage else { return Ok(true) };
        if !self.registered || self.disconnect_attempted || self.peer.is_none() {
            return Err(ParentError::WrongPhase.into());
        }
        let binding = sugarsubstitute_contracts::lifecycle::ClientBinding {
            run: self.run,
            connection: self.connection,
        };
        let mut failed = outage.failed;
        if let Some(handoff) = self.recovery.handoff {
            match resources.backend.invalidated_lease(binding,handoff.pair.current) {
                Ok(_)=>failed=handoff.pair.current,
                Err(cause) if cause.downcast_ref::<sugarsubstitute_contracts::lifecycle::LifecycleRejection>()==Some(&sugarsubstitute_contracts::lifecycle::LifecycleRejection::StaleIdentity)=>{},
                Err(cause)=>return Err(cause),
            }
        }
        let key = resources.backend.recovery_cause_key(binding, failed)?;
        if resources.backend.cleanup_failure(key).is_some() {
            self.backend_failed_resource.get_or_insert(key);
            return Err(ParentError::OwnerFailed.into());
        }
        if resources.backend.failure(key).is_none() {
            return Ok(true);
        }
        if self.acquisition.is_none() {
            return Err(ParentError::WrongPhase.into());
        }
        let identity = self.main.snapshot().identity.ok_or(ParentError::WrongPhase)?;
        let owner = self.backend_causes.as_mut().ok_or(ParentError::WrongPhase)?;
        if owner.contains(key) {
            return Ok(true);
        }
        let slot = owner.free_slot().ok_or(ParentError::Overloaded)?;
        let mut cause = None;
        if !resources.backend.transfer_recovery_failure(key, &mut cause)? {
            if let Some(budget) = self.cleanup
                && resources.execution.client.now()? >= budget.settle_until
            {
                self.backend_failed_resource.get_or_insert(key);
                return Err(ParentError::OwnerFailed.into());
            }
            return Ok(false);
        }
        if let Some(cause) = cause {
            *slot = Some(super::super::attempt::SettledAttemptFailure {
                identity,
                resource: key,
                cause,
            });
        } else {
            return Err(ParentError::WrongPhase.into());
        }
        Ok(true)
    }
}
