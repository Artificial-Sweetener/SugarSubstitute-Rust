// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve the retained primary token's local storage through the native known-folder boundary.
use super::*;
use windows::Win32::{
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG, SHGetKnownFolderPath},
};
/// Resolve and pin the current account folder, preserving native output and original causes.
pub(crate) fn resolve(
    token: HANDLE,
    handles: &mut Vec<Handle>,
    budget: Budget,
) -> Result<PathBuf, ClaimError> {
    // SAFETY: Retained QUERY|IMPERSONATE token and static known-folder GUID remain valid.
    let raw =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KNOWN_FOLDER_FLAG(0), Some(token)) }
            .map_err(|e| ClaimError::new("account_storage", e))?;
    // SAFETY: Successful Shell output is a valid bounded NUL-terminated allocation.
    let known = unsafe { paths::read_native_path(raw, 32767) };
    // SAFETY: This exact Shell allocation is released once with its documented allocator.
    unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    let known = known?;
    let known_handle = paths::open_retained(
        &known,
        true,
        false,
        budget.clone(),
        handles,
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &(budget.clone()),
        )?,
    )?;
    let directory = paths::final_path(
        &handles[known_handle],
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &handles[known_handle].budget(),
        )?,
        crate::durable_storage::protected::native_name::NativeNameDestination::Ordinary,
    )
    .and_then(crate::durable_storage::protected::native_name::NativeName::into_ordinary)?;
    let known_identity = paths::file_identity(
        &handles[known_handle],
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &handles[known_handle].budget(),
        )?,
    )?;
    paths::pin_ancestors(
        &directory,
        handles,
        budget.clone(),
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &(budget.clone()),
        )?,
    )?;
    paths::verify_pinned(
        &directory,
        known_identity,
        handles,
        true,
        budget.clone(),
        &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
            &(budget.clone()),
        )?,
    )?;
    Ok(directory)
}
