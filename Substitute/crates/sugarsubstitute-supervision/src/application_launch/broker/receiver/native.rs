// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Duplicate only exact source-process disk handles; adopt outputs before checking native status.
use super::*;
use windows::Win32::{
    Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle},
    Storage::FileSystem::{FILE_TYPE_DISK, GetFileType},
    System::Threading::GetCurrentProcess,
};

/// Keep every produced duplicate in retained receiver custody, including failed validation outputs.
pub(super) fn duplicate(
    source: HANDLE,
    raw: u64,
    inherit: bool,
    outputs: &mut Vec<Handle>,
    budget: Budget,
) -> Result<usize, ClaimError> {
    let raw =
        usize::try_from(raw).map_err(|cause| ClaimError::new("launch_transfer_handle", cause))?;
    let mut output = HANDLE::default();
    // SAFETY: the caller retains the exact source process and validates a non-pseudo source value.
    // The source owner keeps the declared disk object alive until the correlated terminal reply.
    let duplicated = unsafe {
        DuplicateHandle(
            source,
            HANDLE(raw as *mut _),
            GetCurrentProcess(),
            &mut output,
            0,
            inherit,
            DUPLICATE_SAME_ACCESS,
        )
    };
    if !output.is_invalid() {
        outputs.push(Handle::new(output, budget));
    }
    duplicated.map_err(|cause| ClaimError::new("launch_transfer_duplicate", cause))?;
    if output.is_invalid() {
        return Err(ClaimError::invalid("launch_transfer_duplicate_absent"));
    }
    // SAFETY: the adopted output remains owned in the receiver through this type check.
    if unsafe { GetFileType(output) } != FILE_TYPE_DISK {
        return Err(ClaimError::invalid("launch_transfer_not_disk"));
    }
    Ok(output.0 as usize)
}
