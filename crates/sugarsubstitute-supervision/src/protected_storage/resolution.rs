// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve one root through the sole shared account and canonical path algorithms.
use super::*;
use crate::claim_error::BoundedNativeCause;
use native_phase::NativePhase;

pub(super) enum ScratchRole {
    Account,
    Path,
}

/// Hold the named original scratch while observing both sides of a finite sequence.
pub(super) fn with_phase<T>(
    budget: &Budget,
    causes: &mut NativeCauseDestinations,
    current: &JobContext,
    role: ScratchRole,
    operation: impl FnOnce(&mut NativePhase<'_>) -> Result<T, ClaimError>,
) -> Result<T, ClaimError> {
    match budget {
        Budget::Diagnostic(original) => {
            let mut scratch = match role {
                ScratchRole::Account => original.account_scratch(current),
                ScratchRole::Path => original.path_scratch(current),
            }
            .map_err(ClaimError::diagnostic_allocation)?;
            let mut phase =
                NativePhase::Diagnostic { original, current, scratch: &mut scratch, causes };
            let result = operation(&mut phase);
            phase.post_context(result, current)
        }
        _ => {
            let mut phase = NativePhase::ordinary(budget)?;
            let result = operation(&mut phase);
            phase.post_context(result, current)
        }
    }
}
impl ProtectedStorageRoot {
    /// Pin an existing canonical local root under the actual original operation.
    /// Partial token, file and ancestor outputs remain in the same durable capsule.
    /// # Errors
    /// Rejects repeated use, unbounded capacity, changed context/identity or protection.
    pub fn resolve(&mut self, path: &Path, context: &JobContext) -> Result<(), ClaimError> {
        let state = self.state_mut()?;
        bind_context(state, context, false)?;
        match &state.budget {
            Budget::Diagnostic(original) => {
                original.check(context).map_err(ClaimError::diagnostic_allocation)?
            }
            Budget::Protected(memory) if memory.bytes() >= STORAGE_BYTES => check(context)?,
            _ => return Err(state.budget.rejection("storage_resolution_admission")),
        }
        if !state.handles.is_empty() || state.root.is_some() {
            return Err(state.budget.rejection("storage_resolution_admission"));
        }
        state.causes.check(false)?;
        paths::validate_local_shape(path).map_err(|operation| {
            state.causes.error(
                operation,
                BoundedNativeCause::RawIo(std::io::ErrorKind::InvalidData.into()),
                false,
            )
        })?;
        state.handles.try_reserve_exact(HANDLE_BOUND).map_err(|cause| {
            state.causes.error(
                "storage_handle_reservation",
                BoundedNativeCause::Reserve(cause),
                false,
            )
        })?;
        if state.handles.capacity() > HANDLE_BOUND {
            return Err(state.budget.rejection("storage_handle_capacity"));
        }
        let State { budget, handles, sid, root, causes, .. } = state;
        *sid = with_phase(budget, causes, context, ScratchRole::Account, |phase| {
            account::resolve_current(
                handles,
                budget.clone(),
                account::PrimaryAccountAccess::Query,
                phase,
            )
            .map(|account| account.sid)
        })?;
        let resolved = with_phase(budget, causes, context, ScratchRole::Path, |phase| {
            let index = paths::open_retained(path, true, false, budget.clone(), handles, phase)?;
            let destination = phase.name_destination()?;
            let canonical = paths::final_path(&handles[index], phase, destination)?;
            let identity = paths::file_identity(&handles[index], phase)?;
            paths::pin_ancestors(canonical.path()?, handles, budget.clone(), phase)?;
            paths::verify_pinned(
                canonical.path()?,
                identity,
                handles,
                true,
                budget.clone(),
                phase,
            )?;
            security::verify(&handles[index], sid.bytes()?, phase)?;
            Ok((canonical, identity, index))
        })?;
        *root = Some(resolved);
        Ok(())
    }
}
