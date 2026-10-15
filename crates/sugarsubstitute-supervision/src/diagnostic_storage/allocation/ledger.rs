// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one original claim and account closed destinations before allocation.
mod native_binding;
use super::*;

/// Fixed collection partition of the original protected store claim, in bytes.
pub const DIAGNOSTIC_COLLECTION_BYTES: usize = 393_216;
/// Fixed parser/codec partition of the original protected store claim, in bytes.
pub const DIAGNOSTIC_DECODE_BYTES: usize = 786_432;
/// Fixed remaining partition for actual owner, buffers and escaped results.
pub const DIAGNOSTIC_RESIDUAL_BYTES: usize = 7_208_960;

pub(super) struct Ledger {
    state: Mutex<LedgerState>,
    _fixture: Mutex<Option<FixtureMarker>>,
    _original: MemoryClaim,
}
struct FixtureMarker(Arc<std::sync::atomic::AtomicBool>);
impl Drop for FixtureMarker {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}
struct LedgerState {
    residual: usize,
    collection: bool,
    decode: bool,
    current: Option<LogicalStoreOperationKey>,
    native: Option<loans::Binding>,
    native_generation: u64,
    closed: bool,
    namespace: Option<Arc<crate::coordinator_entry::namespace::NamespaceLifetime>>,
    capture_source: Option<crate::protected_storage::capture_read::pin::ReadPins>,
}
#[derive(Clone)]
pub(in crate::diagnostic_storage) struct Owner {
    inner: Option<Arc<Ledger>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            drop(Arc::into_inner(inner));
        }
    }
}

impl Ledger {
    pub(super) fn admit(original: MemoryClaim) -> Result<Owner, DiagnosticStorageError> {
        if original.bytes() < DIAGNOSTIC_STORAGE_BYTES {
            return Err(DiagnosticStorageError::rejected(None, Code::Capacity));
        }
        let residual = roles::LEDGER_CONTROL_BYTES + roles::OWNER_BASE_BYTES;
        if residual > DIAGNOSTIC_RESIDUAL_BYTES {
            return Err(DiagnosticStorageError::rejected(None, Code::Capacity));
        }
        Ok(Owner {
            inner: Some(Arc::new(Self {
                state: Mutex::new(LedgerState {
                    residual,
                    collection: false,
                    decode: false,
                    current: None,
                    native: None,
                    native_generation: 0,
                    closed: false,
                    namespace: None,
                    capture_source: None,
                }),
                _fixture: Mutex::new(None),
                _original: original,
            })),
        })
    }
    fn lock(&self) -> MutexGuard<'_, LedgerState> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
impl Owner {
    pub(in crate::diagnostic_storage) fn bind_namespace(
        &self,
        request: &ProtectedDiagnosticWriteRequest,
    ) -> Result<(), native::DiagnosticNativeAllocationFailure> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(native::DiagnosticNativeAllocationFailure::namespace_rejected)?;
        let mut state = inner.lock();
        if state.closed
            || state.current != Some(request.operation())
            || !request.lifetime.open.load(std::sync::atomic::Ordering::Acquire)
            || state.namespace.as_ref().is_some_and(|held| !Arc::ptr_eq(held, &request.lifetime))
        {
            return Err(native::DiagnosticNativeAllocationFailure::namespace_rejected());
        }
        if state.namespace.is_none() {
            state.namespace = Some(Arc::clone(&request.lifetime));
        }
        Ok(())
    }
    /// Require the identical bound Entry source and full operation after original control sampling.
    pub(in crate::diagnostic_storage) fn validate_namespace(
        &self,
        lifetime: &Arc<crate::coordinator_entry::namespace::NamespaceLifetime>,
        operation: LogicalStoreOperationKey,
    ) -> Result<(), native::DiagnosticNativeAllocationFailure> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(native::DiagnosticNativeAllocationFailure::namespace_rejected)?;
        let state = inner.lock();
        if state.closed
            || state.current != Some(operation)
            || !lifetime.open.load(std::sync::atomic::Ordering::Acquire)
            || state.namespace.as_ref().is_none_or(|held| !Arc::ptr_eq(held, lifetime))
        {
            return Err(native::DiagnosticNativeAllocationFailure::namespace_rejected());
        }
        Ok(())
    }
    pub(in crate::diagnostic_storage) fn admission_sources_open(&self) -> bool {
        self.inner.as_ref().is_some_and(|inner| {
            let state = inner.lock();
            state
                .namespace
                .as_ref()
                .is_none_or(|namespace| namespace.open.load(std::sync::atomic::Ordering::Acquire))
                && state.capture_source.as_ref().is_none_or(|source| source.open())
        })
    }
    pub(in crate::diagnostic_storage) fn bind_capture_source(
        &self,
        source: crate::protected_storage::capture_read::pin::ReadPins,
    ) -> Result<(), native::DiagnosticNativeAllocationFailure> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(native::DiagnosticNativeAllocationFailure::namespace_rejected)?;
        let mut state = inner.lock();
        if state.closed
            || state.current.is_none()
            || !source.open()
            || state.capture_source.as_ref().is_some_and(|held| !held.same(&source))
        {
            return Err(native::DiagnosticNativeAllocationFailure::namespace_rejected());
        }
        if state.capture_source.is_none() {
            state.capture_source = Some(source);
        }
        Ok(())
    }
    pub(super) fn same(&self, other: &Self) -> bool {
        self.inner
            .as_ref()
            .zip(other.inner.as_ref())
            .is_some_and(|(left, right)| Arc::ptr_eq(left, right))
    }
    pub(in crate::diagnostic_storage) fn bind(
        &self,
        binding: loans::Binding,
    ) -> Result<Option<loans::Binding>, DiagnosticStorageError> {
        let key = binding.key();
        let inner = self.inner.as_ref().ok_or_else(|| error(key, Code::Closed))?;
        let mut state = inner.lock();
        if state.closed {
            return Err(error(key, Code::Closed));
        }
        let generation =
            state.native_generation.checked_add(1).ok_or_else(|| error(key, Code::Capacity))?;
        state.native_generation = generation;
        state.current = Some(key);
        Ok(state.native.replace(binding))
    }
    pub(in crate::diagnostic_storage) fn finish_binding(&self) -> Option<loans::Binding> {
        let inner = self.inner.as_ref()?;
        let mut state = inner.lock();
        state.current = None;
        state.native.take()
    }
    pub(in crate::diagnostic_storage) fn close(&self) {
        if let Some(inner) = &self.inner {
            inner.lock().closed = true;
        }
    }
    pub(super) fn matches(&self, key: LogicalStoreOperationKey) -> bool {
        self.inner.as_ref().is_some_and(|inner| {
            let state = inner.lock();
            !state.closed && state.current == Some(key)
        })
    }
    pub(super) fn residual(
        &self,
        role: roles::ResidualRole,
        key: LogicalStoreOperationKey,
    ) -> Result<Residual, DiagnosticStorageError> {
        let bytes = role.bytes();
        let inner = self.inner.as_ref().ok_or_else(|| error(key, Code::Closed))?;
        let mut state = inner.lock();
        if state.closed || state.current != Some(key) {
            return Err(error(key, Code::Closed));
        }
        let next = state
            .residual
            .checked_add(bytes)
            .filter(|n| *n <= DIAGNOSTIC_RESIDUAL_BYTES)
            .ok_or_else(|| error(key, Code::Capacity))?;
        state.residual = next;
        Ok(Residual { owner: self.clone(), bytes })
    }
    pub(super) fn phase(
        &self,
        phase: Phase,
        key: LogicalStoreOperationKey,
    ) -> Result<PhaseLoan, DiagnosticStorageError> {
        let inner = self.inner.as_ref().ok_or_else(|| error(key, Code::Closed))?;
        let mut state = inner.lock();
        if state.closed || state.current != Some(key) {
            return Err(error(key, Code::Closed));
        }
        let live = match phase {
            Phase::Collection => &mut state.collection,
            Phase::Decode => &mut state.decode,
        };
        if *live {
            return Err(error(key, Code::Capacity));
        }
        *live = true;
        Ok(PhaseLoan { owner: self.clone(), phase })
    }
    pub(super) fn retire_correlation(&self, control: Arc<()>) {
        let final_alias = Arc::into_inner(control).is_some();
        if let Some(inner) = &self.inner {
            let mut state = inner.lock();
            if final_alias {
                if let Some(next) = state.residual.checked_sub(roles::CORRELATION_CONTROL_BYTES) {
                    state.residual = next;
                } else {
                    state.closed = true;
                }
            }
        }
    }
}

pub(in crate::diagnostic_storage) struct Residual {
    pub(super) owner: Owner,
    bytes: usize,
}
impl Residual {
    pub(super) fn adopt_correlation(mut self) {
        self.bytes = 0;
    }
}
impl Drop for Residual {
    fn drop(&mut self) {
        let Some(inner) = &self.owner.inner else {
            return;
        };
        let mut state = inner.lock();
        if let Some(next) = state.residual.checked_sub(self.bytes) {
            state.residual = next;
        } else {
            state.closed = true;
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Phase {
    Collection,
    Decode,
}
pub(super) struct PhaseLoan {
    pub(super) owner: Owner,
    phase: Phase,
}
impl Drop for PhaseLoan {
    fn drop(&mut self) {
        let Some(inner) = &self.owner.inner else {
            return;
        };
        let mut state = inner.lock();
        let live = match self.phase {
            Phase::Collection => &mut state.collection,
            Phase::Decode => &mut state.decode,
        };
        if *live {
            *live = false;
        } else {
            state.closed = true;
        }
    }
}

impl Owner {
    #[cfg(any(test, feature = "boundary-fixtures"))]
    pub(in crate::diagnostic_storage) fn matches_fixture(
        &self,
        marker: &Arc<std::sync::atomic::AtomicBool>,
    ) -> bool {
        self.inner.as_ref().is_some_and(|inner| {
            inner
                ._fixture
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|held| Arc::ptr_eq(&held.0, marker))
        })
    }
    #[cfg(any(test, feature = "boundary-fixtures"))]
    pub(in crate::diagnostic_storage) fn retain_fixture(
        &self,
        marker: &Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), DiagnosticStorageError> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| DiagnosticStorageError::rejected(None, Code::Closed))?;
        let mut held = inner._fixture.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if held.is_some() {
            return Err(DiagnosticStorageError::rejected(None, Code::Busy));
        }
        *held = Some(FixtureMarker(marker.clone()));
        marker.store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }
}
