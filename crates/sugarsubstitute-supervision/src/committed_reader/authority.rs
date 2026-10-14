// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Combine genuine Core reader controls with the actual namespace's live fence.
use super::ProtectedCommittedReadRequest;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::{AdmissionError, JobContext, NativeHandleOutput};
use sugarsubstitute_execution::CommittedReaderOrigin;
struct State {
    namespace: Arc<crate::coordinator_entry::NamespaceLifetime>,
    original: CommittedReaderOrigin,
}
#[repr(C)]
struct Control {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: State,
}
/// Closed no-Weak owner: every alias frees its control before dropping the moved
/// state and LAST original Core3MiB claim. No naked Arc projection is available.
pub(crate) struct ReaderAuthority(Option<Arc<State>>);
impl Clone for ReaderAuthority {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for ReaderAuthority {
    fn drop(&mut self) {
        if let Some(state) = self.0.take() {
            drop(Arc::into_inner(state));
        }
    }
}
impl ReaderAuthority {
    /// Allocate only after the concrete reader constructor preflights this header.
    pub(super) fn new(
        request: &ProtectedCommittedReadRequest,
        original: CommittedReaderOrigin,
    ) -> Self {
        Self(Some(Arc::new(State { namespace: Arc::clone(&request.lifetime), original })))
    }
    pub(super) const fn additional_control_bytes() -> usize {
        size_of::<Control>()
    }
    pub(crate) fn origin(&self) -> Result<&CommittedReaderOrigin, AdmissionError> {
        self.0.as_deref().map(|state| &state.original).ok_or(AdmissionError::InvalidSymbolAdmission)
    }
    pub(crate) fn original_context(&self) -> Result<&JobContext, AdmissionError> {
        self.origin()?.original_context()
    }
    pub(crate) fn check_effect(&self) -> Result<(), AdmissionError> {
        let state = self.0.as_deref().ok_or(AdmissionError::InvalidSymbolAdmission)?;
        state.original.check_effect()?;
        if !state.namespace.open.load(Ordering::Acquire) {
            return Err(AdmissionError::InvalidSymbolAdmission);
        }
        Ok(())
    }
    pub(crate) fn control_storage_bytes(&self) -> usize {
        Self::additional_control_bytes()
            + self.origin().map_or(0, CommittedReaderOrigin::control_storage_bytes)
    }
    pub(crate) fn reserve_output(&self) -> Result<NativeHandleOutput, AdmissionError> {
        self.check_effect()?;
        self.origin()?.reserve_output()
    }
}
