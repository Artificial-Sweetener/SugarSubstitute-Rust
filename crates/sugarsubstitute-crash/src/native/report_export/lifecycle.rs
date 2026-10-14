// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Activate held files without truncating an unverified predecessor, then checked-close.
use super::*;
use windows::Win32::{Foundation::CloseHandle, Storage::FileSystem::*};
/// Preserve the consented held predecessor before no-replace stage activation.
pub(super) fn activate(fields: &mut Fields) -> Result<(), E> {
    if fields.stage_activated || fields.stage_deleted {
        return Err(E::Rejected(C::Identity));
    }
    fields.original.check_effect().map_err(E::Control)?;
    super::files::validate_activation(fields)?;
    if fields.target.handle.is_some() {
        rename(fields.target.raw()?, &fields.backup_path)?;
        fields.target_moved = true;
        fields.original.check_effect().map_err(E::Control)?;
    }
    fields.original.check_effect().map_err(E::Control)?;
    rename(fields.stage.raw()?, &fields.target_path)?;
    fields.stage_activated = true;
    fields.original.check_effect().map_err(E::Control)
}
/// Validate identical original allocation and actual paired cleanup controls.
fn check(fields: &Fields, proof: &ReportExportRetirement) -> Result<(), E> {
    if !proof.matches_origin(&fields.original) {
        return Err(E::Rejected(C::Identity));
    }
    proof.check().map_err(E::Control)
}
/// Adopt accepted rollback/delete/close effects before checking the next cutoff.
pub(super) fn retire(fields: &mut Fields, proof: &ReportExportRetirement) -> Result<(), E> {
    check(fields, proof)?;
    if fields.target_moved && !fields.stage_activated {
        rename(fields.target.raw()?, &fields.target_path)?;
        fields.target_moved = false;
        check(fields, proof)?;
    }
    if fields.stage.handle.is_some() && !fields.stage_activated && !fields.stage_deleted {
        check(fields, proof)?;
        delete(fields.stage.raw()?)?;
        fields.stage_deleted = true;
        check(fields, proof)?;
    }
    if fields.target.handle.is_some()
        && fields.target_moved
        && fields.stage_activated
        && !fields.target_deleted
    {
        check(fields, proof)?;
        delete(fields.target.raw()?)?;
        fields.target_deleted = true;
        check(fields, proof)?;
    }
    close(&mut fields.stage, proof)?;
    close(&mut fields.target, proof)?;
    for slot in fields.parents.iter_mut().rev() {
        close(slot, proof)?;
    }
    Ok(())
}
/// Release output accounting only after physical CloseHandle actually succeeds.
fn close(slot: &mut Slot, proof: &ReportExportRetirement) -> Result<(), E> {
    let Some(handle) = slot.handle else {
        return Ok(());
    };
    proof.check().map_err(E::Control)?;
    // SAFETY: This exact kernel output remains uniquely retained until successful
    // checked CloseHandle. On failure neither handle nor output guard is released.
    unsafe { CloseHandle(handle) }.map_err(E::Native)?;
    slot.handle = None;
    slot.output = None;
    proof.check().map_err(E::Control)
}
/// Mark only the retained exact stage or consented backup inode for deletion.
fn delete(handle: HANDLE) -> Result<(), E> {
    let information = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: Exact held stage/consented predecessor handle has DELETE access;
    // initialized structure stays live. This marks only that held inode for deletion.
    unsafe {
        SetFileInformationByHandle(
            handle,
            FileDispositionInfo,
            (&raw const information).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    }
    .map_err(E::Native)
}
/// Rename the held inode without replacing any unexpected target or backup file.
fn rename(handle: HANDLE, path: &[u16]) -> Result<(), E> {
    let length = path.iter().position(|unit| *unit == 0).ok_or(E::Rejected(C::Destination))?;
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let bytes = offset + length * 2;
    let mut storage = [0_usize; 310];
    if length == 0 || bytes > size_of_val(&storage) {
        return Err(E::Rejected(C::Destination));
    }
    // SAFETY: Aligned fixed storage covers FILE_RENAME_INFO and the complete bounded
    // UTF16 path. Parent deny-delete pins prevent selected-path ancestor substitution;
    // ReplaceIfExists=false preserves any unexpected destination or backup occupant.
    unsafe {
        let information = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*information).Anonymous.ReplaceIfExists = false;
        (*information).FileNameLength = (length * 2) as u32;
        std::ptr::copy_nonoverlapping(
            path.as_ptr(),
            storage.as_mut_ptr().cast::<u8>().add(offset).cast(),
            length,
        );
        SetFileInformationByHandle(handle, FileRenameInfo, storage.as_ptr().cast(), bytes as u32)
    }
    .map_err(E::Native)
}
