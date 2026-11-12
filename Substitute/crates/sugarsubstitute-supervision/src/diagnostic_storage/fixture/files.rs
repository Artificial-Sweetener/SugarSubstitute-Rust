// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own bounded external per-file effects, partial writes and checked fake closure.
use super::*;
pub(super) fn read(
    filesystem: &Filesystem,
    id: NativeId,
    offset: u64,
    output: &mut [u8],
    _: &JobContext,
) -> Result<usize, ClaimError> {
    let disk = lock(&filesystem.0);
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
pub(super) fn write(
    filesystem: &Filesystem,
    id: NativeId,
    offset: u64,
    input: &[u8],
    _: &JobContext,
) -> Result<(), ClaimError> {
    let mut disk = lock(&filesystem.0);
    let index = index(&disk, id)?;
    if !disk.handles[index].writable || !disk.handles[index].live {
        return Err(ClaimError::invalid("fake_readonly_write"));
    }
    let file = disk.handles[index].file;
    if offset as usize != disk.files[file].bytes.len() {
        return Err(ClaimError::invalid("fake_append_offset"));
    }
    disk.files[file].bytes.extend_from_slice(input);
    disk.mutations += 1;
    if disk.faults.partial_write {
        return Err(ClaimError::invalid("fake_partial_write"));
    }
    Ok(())
}
pub(super) fn flush(
    filesystem: &Filesystem,
    id: NativeId,
    _: &JobContext,
) -> Result<DiagnosticReadFacts, ClaimError> {
    let disk = lock(&filesystem.0);
    if disk.faults.flush {
        return Err(ClaimError::invalid("fake_flush"));
    }
    if !disk.handles[index(&disk, id)?].writable {
        return Err(ClaimError::invalid("fake_readonly_flush"));
    }
    Ok(facts(&disk.files[disk.handles[index(&disk, id)?].file]))
}
pub(super) fn delete(
    filesystem: &Filesystem,
    id: NativeId,
    object: DiagnosticObject,
    expected: DiagnosticReadFacts,
    _: &JobContext,
) -> Result<(), ClaimError> {
    let mut disk = lock(&filesystem.0);
    let index = index(&disk, id)?;
    let file = disk.handles[index].file;
    if disk.faults.delete_object == Some(object) {
        return Err(ClaimError::invalid("fake_delete"));
    }
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
pub(super) fn absent(
    filesystem: &Filesystem,
    object: DiagnosticObject,
    _: &JobContext,
) -> Result<bool, ClaimError> {
    let disk = lock(&filesystem.0);
    if disk.faults.absence {
        return Err(ClaimError::invalid("fake_absence"));
    }
    Ok(!disk
        .files
        .iter()
        .any(|file| file.object == object && file.active && file.identity != [0; 24]))
}
pub(super) fn close(
    filesystem: &Filesystem,
    id: NativeId,
    _: &JobContext,
) -> Result<(), ClaimError> {
    let mut disk = lock(&filesystem.0);
    let index = index(&disk, id)?;
    if disk.faults.close
        || disk.faults.close_object == Some(disk.files[disk.handles[index].file].object)
    {
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

pub(super) fn open(
    filesystem: &Filesystem,
    object: DiagnosticObject,
    bound: u64,
    _: &JobContext,
) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
    let mut disk = lock(&filesystem.0);
    let file = disk
        .files
        .iter()
        .position(|file| file.object == object && file.active && file.identity != [0; 24])
        .ok_or_else(|| ClaimError::invalid("fake_missing_file"))?;
    let writable = disk
        .handles
        .iter()
        .find(|output| output.file == file && output.live && output.writable)
        .map(|output| (output.id, output.stage));
    let id = if let Some((id, false)) = writable {
        id
    } else if writable.is_some() {
        return Err(ClaimError::invalid("fake_read_sharing"));
    } else {
        handle(&mut disk, file, filesystem)?
    };
    let facts = facts(&disk.files[file]);
    if let Some(clock) = &disk.after_open {
        clock.observe(disk.after_open_time);
    }
    if disk.faults.partial_open || facts.bytes > bound {
        return Err(ClaimError::invalid("fake_open_after_adoption"));
    }
    Ok((id, facts))
}
pub(super) fn open_inventory(
    filesystem: &Filesystem,
    observed: &crate::diagnostic_storage::inventory::ObservedEntry,
    bound: u64,
    context: &JobContext,
) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
    let object = {
        let disk = lock(&filesystem.0);
        disk.files
            .iter()
            .find(|file| file.identity == observed.identity)
            .map(|file| file.object)
            .ok_or_else(|| ClaimError::invalid("fake_inventory_identity"))?
    };
    let result = open(filesystem, object, bound, context)?;
    if lock(&filesystem.0).faults.changed_inventory
        || result.1.identity != observed.identity
        || result.1.bytes != observed.bytes
    {
        return Err(ClaimError::invalid("fake_changed_inventory"));
    }
    Ok(result)
}
