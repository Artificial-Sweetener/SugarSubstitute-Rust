// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Issue selection once from the real running paired symbol record and original ledger.
use super::{SymbolBundleAdmission, SymbolBundleOrigin, origin::OriginState, purpose::Purpose};
use crate::{
    ExecutionClient,
    admission::{Core, State},
    scope::lock,
};
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{
    AdmissionError, AdmittedJobResources, JobContext, Moment, NativeHandleLease,
    NativeResourceError,
};
pub(super) fn effect(
    core: &Core,
    state: &State,
    observed: &JobContext,
    ledger: &NativeHandleLease,
    now: Moment,
    issued: bool,
    purpose: Purpose,
) -> Result<(), AdmissionError> {
    crate::capture_bundle::validate::live(core, state, observed, now)?;
    let record = crate::capture_bundle::validate::association(core, state, observed)?;
    let resource = state
        .symbol_resources
        .get(&observed.job_id())
        .ok_or(AdmissionError::InvalidSymbolAdmission)?;
    if !crate::symbol_admission::valid_main(record.spec)
        || resource.scope_id != record.scope_id
        || purpose.issued(resource) != issued
        || !resource.observation.matches_lease(ledger)
        || ledger.owner_id() != core.id
        || ledger.original_scope_id() != record.scope_id
        || ledger.original_job_id() != record.spec.id
        || !state.jobs.values().any(|cleanup| {
            cleanup.symbol_cleanup.as_ref().is_some_and(|binding| {
                binding.paired
                    && binding.main_spec == record.spec
                    && binding.main_scope.id == record.scope_id
            })
        })
    {
        return Err(AdmissionError::NativeResources(NativeResourceError::OriginalAdmissionRetired));
    }
    Ok(())
}
impl ExecutionClient {
    /// Issue the sole selected2MiB/96 share of an actual running paired symbol main.
    /// Supplied context provides matching facts only; genuine Core ports are reconstructed.
    /// Clock calls and all final owning destruction occur outside Core locks.
    /// # Errors
    /// Rejects counterfeit memory/native ledgers, retired or unpaired jobs, prior issuance,
    /// current scope/cancellation/deadline failure, original claim failure or clock cause.
    pub fn symbol_bundle_admission(
        &self,
        observed: &JobContext,
    ) -> Result<SymbolBundleAdmission, AdmissionError> {
        Ok(SymbolBundleAdmission { origin: admit(self, observed, Purpose::Selected)? })
    }
}

/// Both fixed purposes share the sole actual record/ledger admission transaction.
pub(super) fn admit(
    client: &ExecutionClient,
    observed: &JobContext,
    purpose: Purpose,
) -> Result<SymbolBundleOrigin, AdmissionError> {
    if !SymbolBundleOrigin::layout_fits() {
        return Err(AdmissionError::InvalidSymbolAdmission);
    }
    let ledger = observed.native_handles().ok_or(AdmissionError::InvalidSymbolAdmission)?.clone();
    {
        let state = lock(&client.core.state);
        // Association and ledger identity precede all provider calls/claim issuance.
        crate::capture_bundle::validate::association(&client.core, &state, observed)?;
        let resource = state
            .symbol_resources
            .get(&observed.job_id())
            .ok_or(AdmissionError::InvalidSymbolAdmission)?;
        if !resource.observation.matches_lease(&ledger) {
            return Err(AdmissionError::InvalidSymbolAdmission);
        }
    }
    let now = client.core.clock.metadata().map_err(AdmissionError::Unexpected)?;
    // Reverse declaration drop releases the mutex before any claim/context on error.
    let memory;
    let selected;
    let original;
    {
        let mut state = lock(&client.core.state);
        effect(&client.core, &state, observed, &ledger, now, false, purpose)?;
        memory = observed
            .memory()
            .claim(purpose.bytes())
            .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
        selected =
            ledger.take_partition(purpose.partition()).map_err(AdmissionError::NativeResources)?;
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
        purpose.mark_issued(
            state
                .symbol_resources
                .get_mut(&observed.job_id())
                .ok_or(AdmissionError::InvalidSymbolAdmission)?,
        );
    }
    Ok(SymbolBundleOrigin(Some(Arc::new(OriginState {
        core: Arc::downgrade(&client.core),
        original,
        ledger,
        selected,
        purpose,
        _memory: memory,
    }))))
}
