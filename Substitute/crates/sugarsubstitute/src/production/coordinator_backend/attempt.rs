// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Release only the original acquisition lease and observe its actual cleanup generation.
use super::*;
use sugarsubstitute_contracts::execution::ScopeId;
impl CoordinatorBackend {
    /// Return only correlation retained from the existing admitted canonical target owner.
    /// A matching digest never substitutes for current readiness, lease or native peer checks.
    pub(crate) fn backend_binding(&self, key: ResourceKey) -> Result<BackendBindingDigest> {
        let record =
            self.records[self.index(key)?].as_ref().ok_or(LifecycleRejection::StaleIdentity)?;
        record
            .backend_binding
            .ok_or_else(|| Box::new(LifecycleRejection::WrongPhase) as Box<dyn Error + Send + Sync>)
    }
    pub(crate) fn cleanup_attempt(
        &mut self,
        binding: ClientBinding,
        lease: LeaseKey,
        original: ShutdownBudget,
        resources: &mut BackendResources<'_>,
    ) -> Result<ResourceTransition> {
        self.validate(resources)?;
        if ShutdownBudget::new(original.started)? != original
            || resources.client.now()? >= original.settle_until
        {
            return Err(LifecycleRejection::InvalidClock.into());
        }
        if binding.run != lease.run || binding.connection != lease.connection {
            return Err(LifecycleRejection::StaleIdentity.into());
        }
        if self.policy.resource_phase(lease.resource)
            == Some(sugarsubstitute_application::supervision::ResourcePhase::Uncertain)
            || self.attempt_settled(lease.resource)
        {
            return Ok(ResourceTransition::default());
        }
        let mut transition = ResourceTransition::default();
        // A whole-run disconnect projects at least this one original lease's
        // release/history/output peak without inventing a request identity.
        self.preflight_policy(
            sugarsubstitute_application::supervision::ResourceStorageInput::Disconnect(binding),
        )?;
        // This lease was retained by the authenticated parent from the real Pending/Granted producer.
        self.policy.release_attempt(binding, lease, &mut transition)?;
        self.output_clients.remove_lease(lease);
        self.release_consumed_output();
        self.effects(&mut transition, &mut None, None, Some(original), None, resources)?;
        Ok(transition)
    }
    pub(crate) fn attempt_settled(&self, key: ResourceKey) -> bool {
        self.policy.resource_phase(key)
            == Some(sugarsubstitute_application::supervision::ResourcePhase::Settled)
            && self
                .records
                .iter()
                .flatten()
                .find(|record| record.key == key)
                .is_none_or(|record| record.settled)
    }
    /// Observe this detached lease or the actual complete native generation settlement.
    pub(crate) fn attempt_released(&self, lease: LeaseKey) -> Result<bool> {
        Ok(self.attempt_settled(lease.resource)
            || self.policy.attempt_released(
                ClientBinding { run: lease.run, connection: lease.connection },
                lease,
            )?)
    }
    /// Inspect the retained genuine finite creator scope, including after settlement.
    pub(crate) fn campaign_scope(&self, key: ResourceKey) -> Option<ScopeId> {
        self.records
            .iter()
            .flatten()
            .find(|record| record.key == key)
            .map(|record| record.scope.id())
    }
    pub(crate) fn take_attempt_failure(
        &mut self,
        key: ResourceKey,
    ) -> Result<Option<LifecycleFailure>> {
        if !self.attempt_settled(key) {
            return Err(LifecycleRejection::WrongPhase.into());
        }
        if self.policy.retains_outage(key) && self.cleanup_failure(key).is_some() {
            return Err(Box::new(LifecycleRejection::WrongPhase));
        }
        Ok(self.policy.take_failure(key)?)
    }
}
