// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Release State before actual provider or native calls while retaining the same capsule.
use super::*;

pub(super) struct StateAccess<'a> {
    shared: &'a Shared,
    held: Option<MutexGuard<'a, State>>,
    key: LogicalStoreOperationKey,
    admitted: bool,
}
impl<'a> StateAccess<'a> {
    pub(super) fn acquire(shared: &'a Shared, key: LogicalStoreOperationKey) -> Self {
        let mut held = lock(shared);
        let admitted = !held.active_access;
        if admitted {
            held.active_access = true;
        }
        Self { shared, held: Some(held), key, admitted }
    }
    pub(super) fn data_mut(&mut self) -> Result<&mut State, DiagnosticStorageError> {
        if !self.admitted {
            return Err(error(self.key, Code::Busy));
        }
        self.held.as_deref_mut().ok_or_else(|| error(self.key, Code::Busy))
    }
    pub(super) fn check(
        &mut self,
        key: LogicalStoreOperationKey,
        context: &JobContext,
        cleanup: bool,
    ) -> Result<(), DiagnosticStorageError> {
        if key != self.key {
            return Err(error(key, Code::InvalidIdentity));
        }
        let state = self.data_mut()?;
        if state.port.is_none() {
            return Err(error(key, Code::Busy));
        }
        let binding = state.operation.as_ref().ok_or_else(|| error(key, Code::Closed))?;
        let original =
            if cleanup { binding.cleanup_context.clone() } else { Some(binding.context.clone()) };
        drop(self.held.take());
        let observation = operation::ControlObservation::sample(original.as_ref(), context);
        self.held = Some(lock(self.shared));
        let state = self.data_mut()?;
        let State { operation, last_observed, closed, .. } = state;
        let binding = operation.as_mut().ok_or_else(|| error(key, Code::Closed))?;
        let result = operation::check_binding_observed(
            binding,
            last_observed,
            closed,
            key,
            context,
            cleanup,
            observation,
        );
        // Context destruction can call an injected provider too.
        drop(self.held.take());
        drop(original);
        self.held = Some(lock(self.shared));
        result?;
        if !cleanup && !self.data_mut()?.allocation.admission_sources_open() {
            return Err(error(key, Code::Closed));
        }
        Ok(())
    }
    pub(super) fn call_native<T>(
        &mut self,
        effect: impl FnOnce(&mut dyn Port) -> T,
    ) -> Result<T, DiagnosticStorageError> {
        let key = self.key;
        let state = self.data_mut()?;
        let owned = state.port.take().ok_or_else(|| error(key, Code::Busy))?;
        state.retiring_handles = owned.live_handles();
        state.retiring_files = owned.retained_files();
        drop(self.held.take());
        let mut capsule = DetachedPort { shared: self.shared, owned: Some(owned), key };
        let result = effect(
            capsule.owned.as_mut().ok_or_else(|| error(key, Code::Closed))?.borrow(Some(key))?,
        );
        capsule.restore()?;
        self.held = Some(lock(self.shared));
        Ok(result)
    }
}
impl Drop for StateAccess<'_> {
    fn drop(&mut self) {
        if self.admitted {
            if let Some(state) = self.held.as_deref_mut() {
                state.active_access = false;
            } else {
                let mut state = lock(self.shared);
                // Unwinding cannot reopen normal admission after a partial effect.
                state.closed = true;
                state.allocation.close();
                state.active_access = false;
            }
        }
    }
}

/// Restore the complete physical owner even when an external provider unwinds.
struct DetachedPort<'a> {
    shared: &'a Shared,
    owned: Option<OwnedPort>,
    key: LogicalStoreOperationKey,
}
impl DetachedPort<'_> {
    fn restore(&mut self) -> Result<(), DiagnosticStorageError> {
        let mut state = lock(self.shared);
        if state.port.is_some() {
            return Err(error(self.key, Code::Busy));
        }
        state.port = self.owned.take();
        state.retiring_handles = 0;
        state.retiring_files = 0;
        Ok(())
    }
}
impl Drop for DetachedPort<'_> {
    fn drop(&mut self) {
        if self.owned.is_some()
            && self.restore().is_err()
            && let Some(owned) = self.owned.take()
        {
            std::mem::forget(owned);
        }
    }
}
