// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain protected account storage and exclusively create one capture generation.
//!
//! Resolution, creation and checked retirement run inside admitted finite execution
//! work. Dropping unresolved state performs no native call and retains its original
//! charge until process exit. Existing generation directories are never initialized
//! or deleted. Borrowed resolved names are observations while this owner remains live.
//! `capture-v1-<namespace><epoch>` versions the host's create-only directory layout.
//! Crashpad database contents remain opaque preserved external authoritative data.

pub(crate) mod account;
mod diagnostic;
mod directories;
pub(crate) mod exclusion;
mod generation;
mod leaf;
pub(crate) mod native_file;
pub(crate) mod native_name;
pub(crate) mod native_phase;
pub(crate) mod paths;
#[cfg(test)]
pub(crate) mod profile_fixture;
pub(crate) mod record_file;
mod resolution;
pub(crate) mod security;
#[cfg(test)]
mod tests;

use crate::{
    ClaimError,
    diagnostic_storage::{
        DiagnosticNativeAllocationFailure, DiagnosticNativeGrant, DiagnosticNativeOwnerLoan,
    },
    native_claims::files::{self, Budget, Handle},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::{
    execution::{ExecutionId, JobContext, MemoryClaim, Moment},
    lifecycle::CoordinatorKey,
};

use native_file::NativeCauseDestinations;
use native_name::{NativeName, NativeSid};
const STORAGE_BYTES: u64 = 1024 * 1024;
const HANDLE_BOUND: usize = 260;

struct State {
    handles: Vec<Handle>,
    budget: Budget,
    sid: NativeSid,
    root: Option<(NativeName, [u8; 24], usize)>,
    causes: NativeCauseDestinations,
    attempted: bool,
    generation: Option<ProtectedStorageGeneration>,
    profile_leaf: bool,
    execution: Option<ExecutionId>,
    start_deadline: Option<Moment>,
    close_deadline: Option<Moment>,
}

/// Own actual protected root/generation handles and their admitted storage.
pub struct ProtectedStorageRoot {
    state: Option<Box<State>>,
    allocation: Option<RootAllocation>,
}
/// Keep the root's actual fields/Vec storage charged until after destruction.
enum RootAllocation {
    Ordinary(Budget),
    Diagnostic { original: DiagnosticNativeGrant, _allocation: DiagnosticNativeOwnerLoan },
}

/// Observe the newly created generation while its owning root remains retained.
pub struct ProtectedStorageGeneration {
    pub(crate) directory: PathBuf,
    pub(crate) database: PathBuf,
    pub(crate) metrics: PathBuf,
    pub(crate) ready: bool,
}
impl ProtectedStorageGeneration {
    /// Borrow the canonical newly created generation name while its proof lives.
    pub fn directory_path(&self) -> &Path {
        &self.directory
    }
    /// Borrow the canonical newly created capture database name; not an authority grant.
    pub fn database_path(&self) -> &Path {
        &self.database
    }
    /// Borrow the canonical newly created metrics name; not an authority grant.
    pub fn metrics_path(&self) -> &Path {
        &self.metrics
    }
}
impl ProtectedStorageRoot {
    /// Describe the actual root Box and its complete fixed native-output table.
    pub(crate) fn diagnostic_owner_layout()
    -> Result<files::NativeRoleLayout, files::NativeLayoutFailure> {
        let cells = std::alloc::Layout::array::<Handle>(HANDLE_BOUND)
            .map_err(|_| files::NativeLayoutFailure::Overflow)?
            .size();
        let bytes = std::alloc::Layout::new::<State>()
            .size()
            .checked_add(cells)
            .ok_or(files::NativeLayoutFailure::Overflow)?;
        Ok(files::NativeRoleLayout { retained: bytes, peak: bytes })
    }
    /// Retain an actual claim before any native storage effect. Resolution requires 1MiB.
    /// # Errors
    /// Rejects a claim smaller than the fixed 1MiB storage allocation bound.
    pub fn new(memory: MemoryClaim) -> Result<Self, ClaimError> {
        Self::from_shared_claim(Arc::new(memory))
    }
    /// Retain a shared actual execution claim before any protected root output.
    /// # Errors
    /// Rejects insufficient owner memory before native root resolution.
    pub(crate) fn from_shared_claim(memory: Arc<MemoryClaim>) -> Result<Self, ClaimError> {
        if memory.bytes() < STORAGE_BYTES {
            return Err(ClaimError::invalid("protected_storage_capacity"));
        }
        let budget = Budget::Protected(memory);
        Ok(Self::reserved(
            budget.clone(),
            NativeCauseDestinations::ordinary(),
            RootAllocation::Ordinary(budget),
        ))
    }
    /// Admit the closed root capsule before its Box or native-output vector allocates.
    /// Allocation custody grants no filesystem authority; resolution uses the actual
    /// original storage operation through the grant's concrete retained Binding.
    pub(crate) fn from_diagnostic_grant(
        original: DiagnosticNativeGrant,
    ) -> Result<Self, DiagnosticNativeAllocationFailure> {
        let allocation = original.root_owner()?;
        let causes = NativeCauseDestinations::admit(&original)?;
        Ok(Self::reserved(
            Budget::Diagnostic(original.clone()),
            causes,
            RootAllocation::Diagnostic { original, _allocation: allocation },
        ))
    }
    fn reserved(
        budget: Budget,
        causes: NativeCauseDestinations,
        allocation: RootAllocation,
    ) -> Self {
        Self {
            state: Some(Box::new(State {
                handles: Vec::new(),
                budget,
                sid: NativeSid::Ordinary(Vec::new()),
                root: None,
                causes,
                attempted: false,
                generation: None,
                profile_leaf: false,
                execution: None,
                start_deadline: None,
                close_deadline: None,
            })),
            allocation: Some(allocation),
        }
    }
    fn state_mut(&mut self) -> Result<&mut State, ClaimError> {
        match (&mut self.state, &self.allocation) {
            (Some(state), _) => Ok(state.as_mut()),
            (None, Some(RootAllocation::Diagnostic { original, .. })) => {
                Err(ClaimError::diagnostic_allocation(original.reject_native_state()))
            }
            (None, Some(RootAllocation::Ordinary(budget))) => {
                Err(budget.rejection("retired_storage"))
            }
            (None, None) => Err(ClaimError::invalid("retired_storage")),
        }
    }
    /// Count actual retained native outputs, including partially resolved custody.
    pub(crate) fn live_handles(&self) -> usize {
        self.state.as_ref().map_or(0, |state| {
            state.handles.iter().filter(|handle| !handle.raw().0.is_null()).count()
        })
    }
    /// Exclusively create this coordinator's capture/database/metrics generation.
    ///
    /// # Errors
    /// Rejects any existing generation, repeated attempt, changed root, security
    /// uncertainty or original context expiry. Created predecessors remain intact;
    /// partial generation names/handles remain owned until checked retirement.
    pub fn create_capture_generation(
        &mut self,
        coordinator: CoordinatorKey,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        let state = self.state.as_mut().ok_or_else(|| ClaimError::invalid("retired_storage"))?;
        generation::create(state, coordinator, context)
    }
    /// Borrow only a completely validated newly created generation.
    pub fn generation(&self) -> Option<&ProtectedStorageGeneration> {
        self.state.as_ref()?.generation.as_ref().filter(|generation| generation.ready)
    }
    /// Observe whether all actual handles were successfully retired.
    pub fn is_retired(&self) -> bool {
        self.state.as_ref().is_none_or(|state| state.handles.is_empty())
    }
    /// Close retained handles under the first latched cleanup deadline; preserve directories.
    ///
    /// # Errors
    /// Cancellation/deadline or checked closure failure retains actual owner state.
    /// Retrying requires another admitted cleanup operation; no elapsed budget is reset.
    pub fn retire(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        if let Some(state) = &mut self.state {
            if !state.budget.is_ordinary() {
                return Err(state.budget.rejection("diagnostic_cleanup_requires_binding"));
            }
            bind_context(state, context, true)?;
            check(context)?;
            for handle in state.handles.iter_mut().rev() {
                check(context)?;
                handle.close()?;
            }
            state.handles.clear();
        }
        self.state = None;
        self.allocation = None;
        Ok(())
    }
}
impl Drop for ProtectedStorageRoot {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            // No handle closure, new allocation or scheduling on the observing thread.
            if !state.handles.iter().any(Handle::is_armed) {
                drop(state);
            } else {
                std::mem::forget(state);
                if let Some(allocation) = self.allocation.take() {
                    std::mem::forget(allocation);
                }
            }
        }
    }
}
fn check(context: &JobContext) -> Result<(), ClaimError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|limit| context.now() >= limit)
    {
        Err(ClaimError::invalid("storage_context_expired"))
    } else {
        Ok(())
    }
}
fn bind_context(state: &mut State, context: &JobContext, cleanup: bool) -> Result<(), ClaimError> {
    if state.execution.is_some_and(|owner| owner != context.owner_id()) {
        return Err(state.budget.rejection("foreign_storage_execution"));
    }
    let deadline = context.deadline().ok_or_else(|| state.budget.rejection("storage_deadline"))?;
    let latched = if cleanup { &mut state.close_deadline } else { &mut state.start_deadline };
    if latched.is_some_and(|old| deadline > old) {
        return Err(state.budget.rejection("renewed_storage_deadline"));
    }
    *latched = Some(latched.map_or(deadline, |old| old.min(deadline)));
    state.execution = Some(context.owner_id());
    Ok(())
}
