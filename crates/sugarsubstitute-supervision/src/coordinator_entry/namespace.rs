// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual namespace/image custody independently of cached proof admission.
use super::{election::ElectionRecordOrigin, types::EntryError};
use crate::{
    ClaimError,
    native_claims::files::Handle,
    protected_storage::{account::ResolvedNamespace, exclusion::HeldFile},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryNativeIdentityFacts, EntrySetupBudget},
    execution::{ExecutionId, MemoryClaim, Moment},
    lifecycle::NamespaceId,
};

pub(super) struct NativeState {
    pub(super) handles: Vec<Handle>,
    pub(super) namespace: Option<ResolvedNamespace>,
    pub(super) image_identity: Option<[u8; 24]>,
    pub(super) election: Option<HeldFile<128>>,
    pub(super) entry_inode: Option<[u8; 24]>,
    pub(super) record_origin: Option<ElectionRecordOrigin>,
    pub(super) memory: Option<Arc<MemoryClaim>>,
    pub(super) close_deadline: Option<Moment>,
    pub(super) retired: bool,
}
impl NativeState {
    pub(super) fn new() -> Self {
        Self {
            handles: Vec::new(),
            namespace: None,
            image_identity: None,
            election: None,
            entry_inode: None,
            record_origin: None,
            memory: None,
            close_deadline: None,
            retired: false,
        }
    }
}
impl Drop for NativeState {
    fn drop(&mut self) {
        if let Some(election) = self.election.take() {
            // Unresolved native exclusion Drop must not invoke native calls on the observer.
            std::mem::forget(election);
        }
        if !self.handles.is_empty() {
            std::mem::forget(std::mem::take(&mut self.handles));
        }
    }
}
pub(crate) struct NamespaceLifetime {
    pub(super) state: Mutex<NativeState>,
    pub(crate) open: AtomicBool,
    pub(super) retired: AtomicBool,
    pub(super) port: Arc<dyn super::native::NativePort>,
}
impl NamespaceLifetime {
    pub(super) fn new(port: Arc<dyn super::native::NativePort>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(NativeState::new()),
            open: AtomicBool::new(true),
            retired: AtomicBool::new(false),
            port,
        })
    }
    pub(crate) fn close_admission(&self) {
        self.open.store(false, Ordering::Release);
    }
}
/// Retain one actual successfully consumed namespace/image proof and its original charge.
/// Native revalidation remains inside finite execution work; cached validation is nonblocking.
pub struct VerifiedEntryNamespace {
    pub(crate) lifetime: Arc<NamespaceLifetime>,
    pub(crate) facts: EntryNativeIdentityFacts,
    pub(crate) installation_identity: [u8; 24],
    pub(crate) execution: ExecutionId,
    pub(crate) budget: EntrySetupBudget,
}
impl VerifiedEntryNamespace {
    /// Observe the actual resolved account/installation namespace.
    pub fn namespace(&self) -> NamespaceId {
        self.facts.namespace()
    }
    /// Observe the actual pinned installation file identity as inert cached metadata.
    pub fn installation_identity(&self) -> [u8; 24] {
        self.installation_identity
    }
    /// Observe the exact execution owner that consumed this retained namespace proof.
    pub fn execution_owner(&self) -> ExecutionId {
        self.execution
    }
    /// Observe the immutable original preparation ancestor; retries never renew it.
    pub fn original_preparation(&self) -> EntrySetupBudget {
        self.budget
    }
    /// Check the cached atomic admission fence without native access or mutex locking.
    /// # Errors
    /// Rejects proof use after its actual owner has closed admission.
    pub fn validate_live(&self) -> Result<(), EntryError> {
        if self.lifetime.open.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(EntryError(ClaimError::invalid("entry_namespace_closed")))
        }
    }
}
