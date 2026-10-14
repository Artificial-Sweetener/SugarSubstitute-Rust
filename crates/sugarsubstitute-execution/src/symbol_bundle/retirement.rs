// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bind close-only selection retirement to the actual activated pre-held cleanup job.
use super::SymbolBundleOrigin;
use crate::{
    ExecutionClient,
    admission::{Core, State},
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, AdmittedJobResources, JobClass, JobContext, Moment, Operation,
};
/// Retain identical target and genuine cleanup ports; terminal ACK never proves native closure.
pub struct SymbolBundleRetirement {
    original: JobContext,
    origin: SymbolBundleOrigin,
    cutoff: Moment,
}
fn validate(
    core: &Core,
    state: &State,
    origin: &SymbolBundleOrigin,
    observed: &JobContext,
    now: Moment,
) -> Result<Moment, AdmissionError> {
    let target = origin.state()?;
    if !std::ptr::eq(target.core.as_ptr(), core) {
        return Err(AdmissionError::InvalidSymbolAdmission);
    }
    crate::capture_bundle::validate::live(core, state, observed, now)?;
    let record = crate::capture_bundle::validate::association(core, state, observed)?;
    let binding = record.symbol_cleanup.as_ref().ok_or(AdmissionError::InvalidSymbolAdmission)?;
    let original = binding.original.ok_or(AdmissionError::InvalidSymbolAdmission)?;
    let resource = state
        .symbol_resources
        .get(&target.original.job_id())
        .ok_or(AdmissionError::InvalidSymbolAdmission)?;
    if record.spec.operation != Operation::Infrastructure
        || record.spec.class != JobClass::InfrastructureLifecycle
        || record.spec.reservation_bytes != crate::symbol_admission::CLEANUP_BYTES
        || record.spec.replacement.is_some()
        || !binding.paired
        || binding.main_spec != target.original.spec()
        || binding.main_scope.id != target.original.scope_id()
        || !original.matches_actual(core.id, record.scope_id, record.spec.id)
        || record.deadline != Some(original.deadline())
        || !target.purpose.issued(resource)
        || !resource.observation.matches_lease(&target.ledger)
    {
        return Err(AdmissionError::InvalidSymbolAdmission);
    }
    if now >= original.deadline() {
        return Err(AdmissionError::DeadlineExpired { deadline: original.deadline(), now });
    }
    Ok(original.deadline())
}
impl ExecutionClient {
    /// Derive close-only proof from the same original activated held cleanup job.
    /// Its first immutable <=15s cutoff is reused; target main may be terminal/acknowledged.
    /// # Errors
    /// Rejects foreign origins, unrelated/unactivated cleanup, changed ledger or current
    /// cleanup controls/deadline/clock failure. Supplied matching clock/cancel ports are ignored.
    pub fn symbol_bundle_retirement(
        &self,
        target: &SymbolBundleOrigin,
        observed: &JobContext,
    ) -> Result<SymbolBundleRetirement, AdmissionError> {
        retire(self, target, observed)
    }
}
impl SymbolBundleRetirement {
    /// Revalidate actual same cleanup record and immutable cutoff outside provider locks.
    /// # Errors
    /// Preserves genuine original cleanup cancellation/deadline/fatal clock cause.
    pub fn check(&self) -> Result<(), AdmissionError> {
        let core = self.origin.state()?.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&core.state);
        let cutoff = validate(&core, &state, &self.origin, &self.original, now)?;
        if cutoff == self.cutoff { Ok(()) } else { Err(AdmissionError::InvalidSymbolAdmission) }
    }
    /// Borrow reconstructed genuine cleanup ports; these cannot renew target authority.
    pub const fn original_context(&self) -> &JobContext {
        &self.original
    }
    /// Return the original first held-cleanup cutoff in its unchanged clock epoch.
    pub const fn cleanup_cutoff(&self) -> Moment {
        self.cutoff
    }
    /// Match only identical original private custody, without native settlement authority.
    pub fn matches_origin(&self, target: &SymbolBundleOrigin) -> bool {
        self.origin.same_origin(target)
    }
}

/// Derive each fixed target's proof through the same actual paired cleanup fence.
pub(crate) fn retire(
    client: &ExecutionClient,
    target: &SymbolBundleOrigin,
    observed: &JobContext,
) -> Result<SymbolBundleRetirement, AdmissionError> {
    let origin = target.state()?;
    if !std::sync::Weak::ptr_eq(&origin.core, &Arc::downgrade(&client.core)) {
        return Err(AdmissionError::InvalidSymbolAdmission);
    }
    {
        let state = lock(&client.core.state);
        crate::capture_bundle::validate::association(&client.core, &state, observed)?;
    }
    let now = client.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
    let original;
    let cutoff;
    {
        let state = lock(&client.core.state);
        cutoff = validate(&client.core, &state, target, observed, now)?;
        let record = crate::capture_bundle::validate::association(&client.core, &state, observed)?;
        original = JobContext::new(
            client.core.id,
            record.scope_id,
            record.spec,
            record.deadline,
            record.job.cancellation(),
            client.core.clock.clone(),
            AdmittedJobResources::new(observed.memory().clone(), None),
        );
    }
    Ok(SymbolBundleRetirement { original, origin: target.clone(), cutoff })
}
