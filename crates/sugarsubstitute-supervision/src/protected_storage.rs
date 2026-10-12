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
mod generation;
pub(crate) mod paths;
pub(crate) mod security;
#[cfg(test)]
mod tests;

use crate::{
    ClaimError,
    native_claims::files::{self, Handle},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use sugarsubstitute_contracts::{
    execution::{ExecutionId, JobContext, MemoryClaim, Moment},
    lifecycle::CoordinatorKey,
};
use windows::Win32::{
    Foundation::HANDLE,
    Security::TOKEN_QUERY,
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

const STORAGE_BYTES: u64 = 1024 * 1024;
const HANDLE_BOUND: usize = 260;

struct State {
    handles: Vec<Handle>,
    memory: Arc<MemoryClaim>,
    sid: Vec<u8>,
    root: Option<(PathBuf, [u8; 24], usize)>,
    attempted: bool,
    generation: Option<ProtectedStorageGeneration>,
    execution: Option<ExecutionId>,
    start_deadline: Option<Moment>,
    close_deadline: Option<Moment>,
}

/// Own actual protected root/generation handles and their admitted storage.
pub struct ProtectedStorageRoot {
    state: Option<Box<State>>,
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
    /// Retain an actual claim before any native storage effect. Resolution requires 1MiB.
    /// # Errors
    /// Rejects a claim smaller than the fixed 1MiB storage allocation bound.
    pub fn new(memory: MemoryClaim) -> Result<Self, ClaimError> {
        if memory.bytes() < STORAGE_BYTES {
            return Err(ClaimError::invalid("protected_storage_capacity"));
        }
        Ok(Self {
            state: Some(Box::new(State {
                handles: Vec::new(),
                memory: Arc::new(memory),
                sid: Vec::new(),
                root: None,
                attempted: false,
                generation: None,
                execution: None,
                start_deadline: None,
                close_deadline: None,
            })),
        })
    }
    /// Resolve and retain an existing local root with exact account/System protection.
    ///
    /// # Errors
    /// Rejects repeated resolution, insufficient actual storage, cancellation/deadline,
    /// uncertain account/path identity, reparses or foreign security. Partial native
    /// outputs stay owned for later checked retirement in a fresh admitted cleanup job.
    pub fn resolve(&mut self, path: &Path, context: &JobContext) -> Result<(), ClaimError> {
        let state = self.state.as_mut().ok_or_else(|| ClaimError::invalid("retired_storage"))?;
        bind_context(state, context, false)?;
        check(context)?;
        if state.memory.bytes() < STORAGE_BYTES || !state.handles.is_empty() || state.root.is_some()
        {
            return Err(ClaimError::invalid("storage_resolution_admission"));
        }
        paths::wide(path)?;
        state
            .handles
            .try_reserve_exact(HANDLE_BOUND)
            .map_err(|e| ClaimError::new("storage_handle_reservation", e))?;
        let budget = files::Budget::Protected(state.memory.clone());
        let mut token = HANDLE::default();
        // SAFETY: Initialized token output is adopted before status inspection.
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
            .map_err(|e| ClaimError::new("primary_token", e));
        if !token.0.is_null() {
            state.handles.push(Handle::new(token, budget.clone()));
        }
        opened?;
        if token.0.is_null() {
            return Err(ClaimError::invalid("missing_primary_token"));
        }
        state.sid = account::token_sid(token)?;
        check(context)?;
        let index = paths::open_retained(path, true, false, budget.clone(), &mut state.handles)?;
        let canonical = paths::final_path(&state.handles[index])?;
        let identity = paths::file_identity(&state.handles[index])?;
        paths::pin_ancestors(&canonical, &mut state.handles, budget.clone())?;
        paths::verify_pinned(&canonical, identity, &mut state.handles, true, budget)?;
        security::verify(&state.handles[index], &state.sid)?;
        check(context)?;
        state.root = Some((canonical, identity, index));
        Ok(())
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
            bind_context(state, context, true)?;
            check(context)?;
            for handle in state.handles.iter_mut().rev() {
                check(context)?;
                handle.close()?;
            }
            state.handles.clear();
        }
        self.state = None;
        Ok(())
    }
}
impl Drop for ProtectedStorageRoot {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            // No handle closure, new allocation or scheduling on the observing thread.
            if state.handles.is_empty() {
                drop(state);
            } else {
                std::mem::forget(state);
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
        return Err(ClaimError::invalid("foreign_storage_execution"));
    }
    let deadline = context.deadline().ok_or_else(|| ClaimError::invalid("storage_deadline"))?;
    let latched = if cleanup { &mut state.close_deadline } else { &mut state.start_deadline };
    if latched.is_some_and(|old| deadline > old) {
        return Err(ClaimError::invalid("renewed_storage_deadline"));
    }
    *latched = Some(latched.map_or(deadline, |old| old.min(deadline)));
    state.execution = Some(context.owner_id());
    Ok(())
}
