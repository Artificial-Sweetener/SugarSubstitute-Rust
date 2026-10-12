// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake only external filesystem effects; the real storage owner runs every transition.
#[path = "observations.rs"]
mod observations;
#[path = "stages.rs"]
mod stages;
use super::*;
#[path = "state.rs"]
mod state;
pub(in crate::diagnostic_storage) use state::Filesystem;
use state::*;
impl Port for Filesystem {
    fn resolve(
        &mut self,
        _: &Path,
        namespace: NamespaceId,
        _: &JobContext,
    ) -> Result<(), ClaimError> {
        lock(&self.0).namespace = Some(namespace);
        Ok(())
    }
    fn acquire_writer(&mut self, _: &JobContext) -> Result<(), ClaimError> {
        Ok(())
    }
    fn scan(
        &mut self,
        _: &JobContext,
    ) -> Result<crate::diagnostic_storage::inventory::Scan, ClaimError> {
        let mut disk = lock(&self.0);
        let mut entries: Vec<_> = disk
            .observations
            .iter()
            .map(|entry| crate::diagnostic_storage::inventory::ObservedEntry {
                digest: entry.digest,
                class: entry.class,
                identity: entry.identity,
                bytes: entry.bytes,
                protected_regular: entry.protected_regular,
                payload_group: entry.payload_group,
            })
            .collect();
        let namespace =
            disk.namespace.ok_or_else(|| ClaimError::invalid("fake_unresolved_namespace"))?;
        for file in disk.files.iter().filter(|file| file.active && file.identity != [0; 24]) {
            if entries.iter().any(|entry| entry.identity == file.identity) {
                continue;
            }
            let (class, digest) = crate::protected_storage::record_file::diagnostic_object_binding(
                file.object,
                namespace,
            );
            entries.push(crate::diagnostic_storage::inventory::ObservedEntry {
                digest,
                class,
                identity: file.identity,
                bytes: file.bytes.len() as u64,
                protected_regular: true,
                payload_group: (class == DiagnosticEntryClass::Payload).then_some(digest),
            });
        }
        disk.stage_scan = disk
            .files
            .iter()
            .enumerate()
            .filter_map(|(index, file)| {
                (!file.active && file.identity != [0; 24]).then_some(file.historical).flatten().map(
                    |historical| {
                        (index, file.object, historical, file.identity, file.bytes.len() as u64)
                    },
                )
            })
            .collect();
        disk.stage_scan_complete = !disk.faults.scan_incomplete;
        for (_, object, historical, identity, bytes) in &disk.stage_scan {
            let digest = crate::protected_storage::record_file::diagnostic_object_binding(
                *object,
                historical.owner().namespace,
            )
            .1;
            entries.push(crate::diagnostic_storage::inventory::ObservedEntry {
                digest,
                class: DiagnosticEntryClass::Stage,
                identity: *identity,
                bytes: *bytes,
                protected_regular: true,
                payload_group: None,
            });
        }
        Ok(crate::diagnostic_storage::inventory::Scan {
            entries,
            complete: !disk.faults.scan_incomplete,
        })
    }
    fn open(
        &mut self,
        object: DiagnosticObject,
        bound: u64,
        _: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        let mut disk = lock(&self.0);
        let file = disk
            .files
            .iter()
            .position(|file| file.object == object && file.active && file.identity != [0; 24])
            .ok_or_else(|| ClaimError::invalid("fake_missing_file"))?;
        let id = handle(&mut disk, file)?;
        let facts = facts(&disk.files[file]);
        if let Some(clock) = &disk.after_open {
            clock.observe(disk.after_open_time);
        }
        if disk.faults.partial_open || facts.bytes > bound {
            return Err(ClaimError::invalid("fake_open_after_adoption"));
        }
        Ok((id, facts))
    }
    fn open_inventory(
        &mut self,
        observed: &crate::diagnostic_storage::inventory::ObservedEntry,
        bound: u64,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        let object = {
            let disk = lock(&self.0);
            disk.files
                .iter()
                .find(|file| file.identity == observed.identity)
                .map(|file| file.object)
                .ok_or_else(|| ClaimError::invalid("fake_inventory_identity"))?
        };
        let result = self.open(object, bound, context)?;
        if lock(&self.0).faults.changed_inventory
            || result.1.identity != observed.identity
            || result.1.bytes != observed.bytes
        {
            return Err(ClaimError::invalid("fake_changed_inventory"));
        }
        Ok(result)
    }
    fn create_stage(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        _: u64,
        _: &JobContext,
    ) -> Result<NativeId, ClaimError> {
        let mut disk = lock(&self.0);
        let file = disk.files.len();
        disk.files.push(File {
            object,
            bytes: Vec::new(),
            identity: identity(file),
            active: false,
            historical: Some(historical),
        });
        disk.mutations += 1;
        let id = handle(&mut disk, file)?;
        if disk.faults.partial_stage {
            return Err(ClaimError::invalid("fake_stage_after_adoption"));
        }
        Ok(id)
    }
    fn recover_stage(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        self.recover_existing(object, historical, expected, context)
    }
    fn read(
        &mut self,
        id: NativeId,
        offset: u64,
        output: &mut [u8],
        _: &JobContext,
    ) -> Result<usize, ClaimError> {
        let disk = lock(&self.0);
        let file = disk.handles[index(&disk, id)?].file;
        let end = offset as usize + output.len();
        let source = disk.files[file]
            .bytes
            .get(offset as usize..end)
            .ok_or_else(|| ClaimError::invalid("fake_read_range"))?;
        output.copy_from_slice(source);
        if disk.faults.corrupt_read && !output.is_empty() {
            output[0] ^= 1;
        }
        Ok(output.len())
    }
    fn write(
        &mut self,
        id: NativeId,
        offset: u64,
        input: &[u8],
        _: &JobContext,
    ) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        let file = disk.handles[index(&disk, id)?].file;
        if offset as usize != disk.files[file].bytes.len() {
            return Err(ClaimError::invalid("fake_append_offset"));
        }
        disk.files[file].bytes.extend_from_slice(input);
        disk.mutations += 1;
        Ok(())
    }
    fn flush(&mut self, id: NativeId, _: &JobContext) -> Result<DiagnosticReadFacts, ClaimError> {
        let disk = lock(&self.0);
        Ok(facts(&disk.files[disk.handles[index(&disk, id)?].file]))
    }
    fn activate(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        let actual = self.flush(id, context)?;
        {
            let mut disk = lock(&self.0);
            let current = disk.handles[index(&disk, id)?].file;
            let previous = disk.files.iter().position(|file| {
                file.object == object
                    && file.active
                    && file.identity != [0; 24]
                    && !std::ptr::eq(file, &disk.files[current])
            });
            match (expected, previous) {
                (DiagnosticPredecessor::Absent, None) => (),
                (DiagnosticPredecessor::Exact { file_identity, sha256 }, Some(old))
                    if facts(&disk.files[old]).identity == file_identity
                        && facts(&disk.files[old]).sha256 == sha256 => {}
                _ => return Err(ClaimError::invalid("fake_changed_predecessor")),
            }
            if let Some(old) = previous {
                disk.files[old].identity = [0; 24];
            }
            disk.files[current].active = true;
            disk.mutations += 1;
            if disk.faults.activate {
                return Err(ClaimError::invalid("fake_post_rename_failure"));
            }
        }
        self.close(id, context)?;
        Ok(actual)
    }
    fn reconcile_activation(
        &mut self,
        object: DiagnosticObject,
        _: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        let (id, facts) = self.open(object, object.bound(), context)?;
        self.close(id, context)?;
        Ok(facts)
    }
    fn delete(
        &mut self,
        id: NativeId,
        _: DiagnosticObject,
        expected: DiagnosticReadFacts,
        _: &JobContext,
    ) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        let index = index(&disk, id)?;
        let file = disk.handles[index].file;
        if disk.handles[index].deleted {
            return Ok(());
        }
        if facts(&disk.files[file]) != expected {
            return Err(ClaimError::invalid("fake_changed_delete"));
        }
        disk.handles[index].deleted = true;
        disk.mutations += 1;
        Ok(())
    }
    fn absent(&mut self, object: DiagnosticObject, _: &JobContext) -> Result<bool, ClaimError> {
        let disk = lock(&self.0);
        if disk.faults.absence {
            return Err(ClaimError::invalid("fake_absence"));
        }
        Ok(!disk
            .files
            .iter()
            .any(|file| file.object == object && file.active && file.identity != [0; 24]))
    }
    fn close(&mut self, id: NativeId, _: &JobContext) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        let index = index(&disk, id)?;
        if disk.faults.close {
            return Err(ClaimError::invalid("fake_close"));
        }
        disk.handles[index].live = false;
        disk.handles[index].memory = None;
        if disk.handles[index].deleted {
            let file = disk.handles[index].file;
            disk.files[file].identity = [0; 24];
        }
        Ok(())
    }
    fn retire(&mut self, context: &JobContext) -> Result<(), ClaimError> {
        let ids: Vec<_> = lock(&self.0)
            .handles
            .iter()
            .filter(|handle| handle.live)
            .map(|handle| handle.id)
            .collect();
        for id in ids {
            self.close(id, context)?;
        }
        Ok(())
    }
    fn live_handles(&self) -> usize {
        lock(&self.0).handles.iter().filter(|handle| handle.live).count()
    }
}
fn lock(shared: &Arc<Mutex<Disk>>) -> MutexGuard<'_, Disk> {
    shared.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
