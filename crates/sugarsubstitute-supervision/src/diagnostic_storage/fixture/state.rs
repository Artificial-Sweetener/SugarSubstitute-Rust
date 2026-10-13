// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual fake output custody and its original execution memory claim.
use super::*;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Default)]
pub(in crate::diagnostic_storage) struct Faults {
    pub(in crate::diagnostic_storage) partial_stage: bool,
    pub(in crate::diagnostic_storage) partial_write: bool,
    pub(in crate::diagnostic_storage) flush: bool,
    pub(in crate::diagnostic_storage) partial_open: bool,
    pub(in crate::diagnostic_storage) close: bool,
    pub(in crate::diagnostic_storage) close_object: Option<DiagnosticObject>,
    pub(in crate::diagnostic_storage) delete_object: Option<DiagnosticObject>,
    pub(in crate::diagnostic_storage) activate: bool,
    pub(in crate::diagnostic_storage) absence: bool,
    pub(in crate::diagnostic_storage) scan_incomplete: bool,
    pub(in crate::diagnostic_storage) changed_inventory: bool,
    pub(in crate::diagnostic_storage) corrupt_read: bool,
}
pub(in crate::diagnostic_storage) struct File {
    pub(super) object: DiagnosticObject,
    pub(super) bytes: Vec<u8>,
    pub(super) identity: [u8; 24],
    pub(super) active: bool,
    pub(super) historical: Option<LogicalStoreOperationKey>,
}
pub(super) struct Handle {
    pub(super) id: u64,
    pub(super) file: usize,
    pub(super) live: bool,
    pub(super) deleted: bool,
    pub(super) writable: bool,
    pub(super) stage: bool,
    pub(super) memory: Option<allocation::DiagnosticAllocationOwner>,
}
pub(in crate::diagnostic_storage) struct Disk {
    pub(super) observe_open_context: bool,
    pub(super) files: Vec<File>,
    pub(super) handles: Vec<Handle>,
    pub(in crate::diagnostic_storage) faults: Faults,
    pub(in crate::diagnostic_storage) observations:
        Vec<crate::diagnostic_storage::inventory::ObservedEntry>,
    pub(in crate::diagnostic_storage) mutations: usize,
    pub(in crate::diagnostic_storage) after_open: Option<Arc<dyn BoundaryClock>>,
    pub(in crate::diagnostic_storage) after_open_time: u64,
    pub(super) next: u64,
    pub(super) owned: Arc<AtomicBool>,
    pub(super) namespace: Option<NamespaceId>,
    pub(super) stage_scan: Vec<(usize, DiagnosticObject, LogicalStoreOperationKey, [u8; 24], u64)>,
    pub(super) stage_scan_complete: bool,
}
#[derive(Clone)]
pub(in crate::diagnostic_storage) struct Filesystem(
    pub(in crate::diagnostic_storage) Arc<Mutex<Disk>>,
    pub(super) Option<allocation::DiagnosticAllocationOwner>,
);
impl Filesystem {
    pub(in crate::diagnostic_storage) fn new() -> Self {
        Self(
            Arc::new(Mutex::new(Disk {
                observe_open_context: false,
                files: Vec::new(),
                handles: Vec::new(),
                faults: Faults::default(),
                observations: Vec::new(),
                mutations: 0,
                after_open: None,
                after_open_time: 0,
                next: 1,
                owned: Arc::new(AtomicBool::new(false)),
                namespace: None,
                stage_scan: Vec::new(),
                stage_scan_complete: false,
            })),
            None,
        )
    }
    pub(in crate::diagnostic_storage) fn retain_claim(
        &self,
        owner: &allocation::DiagnosticAllocationOwner,
    ) -> Result<Self, ClaimError> {
        let disk = lock(&self.0);
        if disk.handles.iter().any(|handle| handle.live) || disk.owned.load(Ordering::Acquire) {
            return Err(ClaimError::invalid("fake_live_owner"));
        }
        owner
            .retain_fixture(&disk.owned)
            .map_err(|cause| ClaimError::new("fake_original_owner", cause))?;
        Ok(Self(self.0.clone(), Some(owner.clone())))
    }
    #[cfg(test)]
    pub(in crate::diagnostic_storage) fn erase_existing(&self, object: DiagnosticObject) {
        let mut disk = lock(&self.0);
        for file in &mut disk.files {
            if file.object == object {
                file.identity = [0; 24];
            }
        }
    }
    #[cfg(test)]
    pub(in crate::diagnostic_storage) fn put(
        &self,
        object: DiagnosticObject,
        bytes: &[u8],
    ) -> DiagnosticReadFacts {
        let mut disk = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let identity = identity(disk.files.len());
        let index = disk.files.len();
        disk.files.push(File {
            object,
            bytes: bytes.to_vec(),
            identity,
            active: true,
            historical: None,
        });
        facts(&disk.files[index])
    }
}
pub(super) fn identity(index: usize) -> [u8; 24] {
    if index < 255 {
        return [(index + 1) as u8; 24];
    }
    let mut value = [1; 24];
    value[..8].copy_from_slice(&((index + 1) as u64).to_le_bytes());
    value
}
pub(super) fn facts(file: &File) -> DiagnosticReadFacts {
    DiagnosticReadFacts {
        identity: file.identity,
        sha256: Sha256::digest(&file.bytes).into(),
        bytes: file.bytes.len() as u64,
    }
}
pub(super) fn handle(disk: &mut Disk, file: usize, owner: &Filesystem) -> Result<u64, ClaimError> {
    if disk.handles.iter().filter(|handle| handle.live).count() >= 16 {
        return Err(ClaimError::invalid("fake_handle_capacity"));
    }
    let memory = owner.1.clone().ok_or_else(|| ClaimError::invalid("fake_missing_claim"))?;
    let id = disk.next;
    disk.next =
        disk.next.checked_add(1).ok_or_else(|| ClaimError::invalid("fake_output_id_exhausted"))?;
    let output = Handle {
        id,
        file,
        live: true,
        deleted: false,
        writable: false,
        stage: false,
        memory: Some(memory),
    };
    if let Some(old) = disk.handles.iter_mut().find(|handle| !handle.live) {
        *old = output;
    } else {
        disk.handles.push(output);
    }
    Ok(id)
}
pub(super) fn index(disk: &Disk, id: u64) -> Result<usize, ClaimError> {
    disk.handles
        .iter()
        .position(|handle| handle.id == id)
        .ok_or_else(|| ClaimError::invalid("fake_retired_handle"))
}
