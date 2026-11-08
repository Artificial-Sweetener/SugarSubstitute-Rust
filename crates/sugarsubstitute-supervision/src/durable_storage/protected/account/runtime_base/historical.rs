// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read the actual primary account's historical location without provisioning a known folder.
use super::*;
use std::{ffi::OsString, os::windows::ffi::OsStringExt};
use windows::{
    Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DONT_VERIFY},
    },
    core::{GUID, HRESULT, PWSTR},
};

#[link(name = "shell32")]
unsafe extern "system" {
    #[link_name = "SHGetKnownFolderPath"]
    fn known_folder(folder: *const GUID, flags: u32, token: HANDLE, output: *mut PWSTR) -> HRESULT;
}

/// Adopt Shell output before HRESULT inspection: its allocation must be freed even on failure.
/// The generated Result-returning binding discards that failure output, so this exact ABI keeps
/// the caller-owned output cell visible until its documented CoTaskMemFree retirement.
struct ShellPath(PWSTR);
impl Drop for ShellPath {
    fn drop(&mut self) {
        // SAFETY: This is precisely the Shell-owned output cell, released once on every outcome.
        unsafe { CoTaskMemFree(Some(self.0.0.cast())) };
    }
}

/// Resolve only native primary-account facts; text environments never choose a migration source.
/// This observation grants no creation, deletion, permission repair or claim activation authority.
/// # Errors
/// Retains original token/Shell/path failures and rejects a changed account or oversized result.
pub(crate) fn resolve(
    handles: &mut Vec<Handle>,
    budget: Budget,
    expected_sid: &[u8],
) -> Result<PathBuf, ClaimError> {
    let token_slot = handles.len();
    let mut phase = NativePhase::ordinary(&budget)?;
    let account = resolve_current(
        handles,
        budget.clone(),
        PrimaryAccountAccess::HistoricalStorage,
        &mut phase,
    )?;
    if account.sid.bytes()? != expected_sid {
        return Err(phase.invalid("historical_account_changed"));
    }
    let mut output = ShellPath(PWSTR::null());
    phase.check()?;
    // SAFETY: The actual primary token remains retained with QUERY|IMPERSONATE rights. Only
    // DONT_VERIFY is passed, never CREATE/INIT. The initialized output is owned before status.
    let status = unsafe {
        known_folder(
            &FOLDERID_LocalAppData,
            KF_FLAG_DONT_VERIFY.0 as u32,
            handles[token_slot].raw(),
            &mut output.0,
        )
    };
    status.ok().map_err(|cause| {
        phase.error("historical_account_folder", BoundedNativeCause::Windows(cause))
    })?;
    if output.0.is_null() {
        return Err(phase.invalid("historical_account_folder_missing"));
    }
    let mut length = 0;
    while length < 32767 {
        // SAFETY: Successful Shell output is a NUL-terminated path allocation. Windows paths
        // are bounded to32767 units, and the owning ShellPath stays live through the decode.
        if unsafe { output.0.0.add(length).read() } == 0 {
            break;
        }
        length += 1;
    }
    if length == 0 || length == 32767 {
        return Err(phase.invalid("historical_account_folder_bound"));
    }
    // SAFETY: The successful native path's complete non-NUL extent was observed while owned.
    let units = unsafe { std::slice::from_raw_parts(output.0.0, length) };
    let path = PathBuf::from(OsString::from_wide(units));
    crate::durable_storage::files::lexical(&path)
        .map_err(|cause| ClaimError::new("historical_account_folder_shape", cause))?;
    paths::validate_local_shape(&path).map_err(ClaimError::invalid)?;
    phase.check()?;
    Ok(path)
}
