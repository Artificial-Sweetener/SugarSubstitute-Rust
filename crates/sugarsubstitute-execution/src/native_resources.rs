// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain original admitted symbol counter provenance independently of terminal delivery.

use crate::{
    admission::{Core, State},
    scope::lock,
};
use std::sync::Arc;
mod original;
pub(crate) use original::SymbolOriginalContext;
use sugarsubstitute_contracts::execution::{
    JobId, MemoryClaim, MemoryLease, NativeHandleObservation, NativeHandleRelease, ScopeId,
};

#[cfg(any(windows, test))]
use sugarsubstitute_contracts::execution::CancelControl;

#[cfg(any(windows, test))]
#[derive(Clone)]
pub(crate) struct EffectContext {
    core: std::sync::Weak<Core>,
    original: SymbolOriginalContext,
    child: Option<sugarsubstitute_contracts::execution::JobContext>,
    handles: Arc<sugarsubstitute_contracts::execution::NativeHandleClaim>,
}
#[cfg(any(windows, test))]
impl EffectContext {
    pub(crate) fn new(reservation: &crate::SymbolTransportReservation) -> Self {
        Self {
            core: reservation.core.clone(),
            original: reservation.original.clone(),
            child: None,
            handles: reservation.handles.clone(),
        }
    }
    /// Bind the real native child context before its first finite native effect.
    pub(crate) fn for_child(
        &self,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Self {
        Self {
            core: self.core.clone(),
            original: self.original.clone(),
            child: Some(context.clone()),
            handles: self.handles.clone(),
        }
    }
    /// Sample externally before locks, then validate the actual original ledger and live record.
    pub(crate) fn check(&self) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        use sugarsubstitute_contracts::execution::AdmissionError;
        let core = self.core.upgrade().ok_or(AdmissionError::ShuttingDown)?;
        let now = core.clock.metadata().map_err(AdmissionError::Unexpected)?;
        let state = lock(&core.state);
        crate::symbol_admission::validate_context(
            &core,
            &state,
            &self.original,
            &self.handles,
            now,
        )?;
        if let Some(child) = &self.child {
            let invalid = || {
                AdmissionError::NativeResources(sugarsubstitute_contracts::execution::NativeResourceError::OriginalAdmissionRetired)
            };
            let record = state.jobs.get(&child.job_id()).ok_or_else(invalid)?;
            let scope = state.scopes.get(&child.scope_id()).ok_or_else(invalid)?;
            if child.owner_id() != core.id
                || record.scope_id != child.scope_id()
                || record.spec != child.spec()
                || record.deadline != child.deadline()
                || record.symbol_original != Some(self.original.job_id())
                || record.phase != sugarsubstitute_contracts::execution::JobPhase::Running
                || record.retiring
            {
                return Err(invalid());
            }
            if let Some(deadline) =
                record.native_deadlines.as_ref().and_then(|slots| slots.expired_deadline(now))
            {
                return Err(AdmissionError::DeadlineExpired { deadline, now });
            }
            if scope.closed
                || scope.cancel.is_requested()
                || record.job.cancellation().is_requested()
                || child.cancellation_requested()
                || record.native_deadlines.as_ref().is_none_or(|slots| slots.stopped())
            {
                return Err(AdmissionError::ScopeClosed { scope_id: child.scope_id() });
            }
        }
        Ok(())
    }
}

pub(crate) struct ResourceRecord {
    pub(crate) bundle_issued: bool,
    pub(crate) reader_issued: bool,
    pub(crate) provider_issued: bool,
    pub(crate) scope_id: ScopeId,
    pub(crate) observation: NativeHandleObservation,
}

struct Release {
    core: Arc<Core>,
    job_id: JobId,
    // The weak Core record cannot retain its allocation's original byte charge.
    _metadata: MemoryClaim,
}
impl NativeHandleRelease for Release {
    fn release_symbol_reservation(&self) {
        let removed = {
            let mut state = lock(&self.core.state);
            let removed = state.symbol_resources.remove(&self.job_id);
            Core::retire_scopes(&mut state);
            removed
        };
        drop(removed);
        self.core.changed();
    }
    fn output_custody_changed(&self) {
        self.core.changed();
    }
}

/// Original fixed metadata covers the real ledger, release, logical record and
/// unique closed original-context control. Context/claim clones allocate nothing.
pub(crate) const SYMBOL_METADATA_BYTES: u64 = 4096;

/// Preflight each concrete covered allocation before the original metadata claim.
pub(crate) const fn symbol_storage_bytes() -> usize {
    #[repr(C)]
    struct ClaimControl {
        strong: std::sync::atomic::AtomicUsize,
        weak: std::sync::atomic::AtomicUsize,
        claim: sugarsubstitute_contracts::execution::NativeHandleClaim,
    }
    size_of::<Release>()
        + size_of::<ResourceRecord>()
        + sugarsubstitute_contracts::execution::NativeHandleLease::control_storage_bytes()
        + size_of::<ClaimControl>()
        + SymbolOriginalContext::control_storage_bytes()
}

pub(crate) fn release(
    core: Arc<Core>,
    job_id: JobId,
    memory: &MemoryLease,
) -> Result<Box<dyn NativeHandleRelease>, sugarsubstitute_contracts::execution::MemoryClaimError> {
    let required = symbol_storage_bytes() as u64;
    if required > SYMBOL_METADATA_BYTES {
        return Err(sugarsubstitute_contracts::execution::MemoryClaimError {
            requested_bytes: required,
            available_bytes: SYMBOL_METADATA_BYTES,
        });
    }
    Ok(Box::new(Release { core, job_id, _metadata: memory.claim(SYMBOL_METADATA_BYTES)? }))
}

/// Observe only passive counters; zero does not prove checked native retirement.
pub(crate) fn output_custody(state: &State) -> Vec<crate::NativeOutputCustody> {
    let mut retained = state
        .symbol_resources
        .iter()
        .filter_map(|(job_id, resource)| {
            let (outputs_in_custody, high_water) = resource.observation.snapshot();
            (outputs_in_custody != 0).then_some(crate::NativeOutputCustody {
                original_scope_id: resource.scope_id,
                original_job_id: *job_id,
                outputs_in_custody,
                high_water,
            })
        })
        .collect::<Vec<_>>();
    retained.extend(crate::report_export::resources::output_custody(state));
    retained.sort_unstable_by_key(|row| (row.original_scope_id, row.original_job_id));
    retained
}
