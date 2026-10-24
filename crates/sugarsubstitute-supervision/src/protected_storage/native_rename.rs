// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish retained filesystem objects through one bounded native rename adapter.
use crate::{ClaimError, native_claims::files::Handle};
use windows::Win32::Storage::FileSystem::{
    FILE_RENAME_INFO, FileRenameInfo, SetFileInformationByHandle,
};

/// Select an absolute destination or a name within the source object's verified parent.
pub(crate) enum Target<'a> {
    Absolute(&'a std::path::Path),
    SameDirectory(&'a std::path::Path),
}
/// Rename through retained source and parent handles using bounded owned UTF16.
/// # Errors
/// Preserve source custody on invalid target shape or native activation failure.
pub(crate) fn rename(
    handle: &Handle,
    target: Target<'_>,
    replace: bool,
    phase: &mut crate::protected_storage::native_phase::NativePhase<'_>,
) -> Result<(), ClaimError> {
    use crate::claim_error::BoundedNativeCause;
    phase.check_budget(&handle.budget())?;
    let (target, same_directory) = match target {
        Target::Absolute(path) => (path, false),
        Target::SameDirectory(path) => (path, true),
    };
    let mut producing = phase.wide(target)?;
    let total_units = producing.units().len() - 1;
    let start = if same_directory {
        use std::os::windows::ffi::OsStrExt;
        let leaf = target.file_name().ok_or_else(|| {
            producing.error(
                "record_rename_leaf",
                BoundedNativeCause::RawIo(std::io::Error::from(std::io::ErrorKind::InvalidInput)),
            )
        })?;
        total_units - leaf.encode_wide().count()
    } else {
        0
    };
    let units = total_units - start;
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let bytes = offset + producing.units().len() * 2;
    let count = bytes.div_ceil(size_of::<usize>());
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(count).map_err(|cause| {
        producing.error("record_rename_reservation", BoundedNativeCause::Reserve(cause))
    })?;
    if buffer.capacity() > count {
        return Err(producing.error(
            "record_rename_capacity",
            BoundedNativeCause::RawIo(std::io::Error::from(std::io::ErrorKind::InvalidData)),
        ));
    }
    buffer.resize(count, 0_usize);
    producing.check()?;
    // SAFETY: Aligned owned buffer contains FILE_RENAME_INFO and the complete,
    // already bounded UTF16 destination. Source custody stays retained through the
    // synchronous call; same-parent mode uses the native simple-name contract.
    let result = unsafe {
        let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*information).Anonymous.ReplaceIfExists = replace;
        (*information).FileNameLength = (units * 2) as u32;
        std::ptr::copy_nonoverlapping(
            producing.units().as_ptr().add(start),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast(),
            units + 1,
        );
        if same_directory {
            use windows::Wdk::Storage::FileSystem::{FileRenameInformation, NtSetInformationFile};
            use windows::Win32::{Foundation::RtlNtStatusToDosError, System::IO::IO_STATUS_BLOCK};
            let mut io = IO_STATUS_BLOCK::default();
            // A simple native name with null RootDirectory renames within the source's
            // actual parent. Absolute Win32 names reopen that pinned parent for writes.
            let status = NtSetInformationFile(
                handle.raw(),
                &mut io,
                buffer.as_ptr().cast(),
                bytes as u32,
                FileRenameInformation,
            );
            if status.is_ok() {
                Ok(())
            } else {
                Err(BoundedNativeCause::RawIo(std::io::Error::from_raw_os_error(
                    RtlNtStatusToDosError(status) as i32,
                )))
            }
        } else {
            SetFileInformationByHandle(
                handle.raw(),
                FileRenameInfo,
                buffer.as_ptr().cast(),
                bytes as u32,
            )
            .map_err(BoundedNativeCause::Windows)
        }
    };
    drop(buffer);
    drop(producing);
    // The caller adopts the actual new path before its unconditional post-control
    // observation. A late return must not leave the retained owner at the old path.
    result.map_err(|cause| phase.error("activate_claim_record", cause))
}
