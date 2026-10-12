// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Duplicate only the current fixed fixture's inherited standard roles under admission.
use super::{NativeLease, RegistryError};
use windows::Win32::{
    Foundation::{
        DUPLICATE_HANDLE_OPTIONS, DuplicateHandle, GetHandleInformation, GetLastError, HANDLE,
        HANDLE_FLAG_INHERIT, SetLastError, WIN32_ERROR,
    },
    Storage::FileSystem::{FILE_TYPE_CHAR, FILE_TYPE_PIPE, FILE_TYPE_UNKNOWN, GetFileType},
    System::{
        Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        Threading::GetCurrentProcess,
    },
};
impl NativeLease {
    pub(super) fn prepare_stdio(&mut self) -> Result<(), RegistryError> {
        for (i, selector) in
            [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE].into_iter().enumerate()
        {
            if self.stdio[i].is_some() {
                return Err(RegistryError::invalid("fixture_output_live_stdio_reuse"));
            }
            // SAFETY: fixed current-process standard roles only. These are borrowed,
            // never adopted/closed; unique restricted duplicates become durable below.
            let original = unsafe { GetStdHandle(selector) }
                .map_err(|cause| RegistryError::new("fixture_output_standard_role", cause))?;
            let mut flags = 0;
            // SAFETY: the borrowed current-process handle remains live; output is initialized.
            unsafe { GetHandleInformation(original, &mut flags) }
                .map_err(|cause| RegistryError::new("fixture_output_source_flags", cause))?;
            // SAFETY: query only the live borrowed handle and this worker's thread
            // error. Clear stale error and capture failure before any cleanup.
            let (kind, error) = unsafe {
                SetLastError(WIN32_ERROR(0));
                let kind = GetFileType(original);
                (kind, GetLastError())
            };
            if kind == FILE_TYPE_UNKNOWN && error.0 != 0 {
                return Err(RegistryError::new(
                    "fixture_output_source_type",
                    windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(error.0)),
                ));
            }
            validate_role(i, flags, kind.0)?;
            let mut duplicate = HANDLE::default();
            // SAFETY: FILE_READ_DATA/FILE_WRITE_DATA plus SYNCHRONIZE; no broad same-access
            // duplication. Stable output is retained before any fallible validation.
            let result = unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    original,
                    GetCurrentProcess(),
                    &mut duplicate,
                    0x00100000 | if i == 0 { 1 } else { 2 },
                    true,
                    DUPLICATE_HANDLE_OPTIONS(0),
                )
            };
            if !duplicate.is_invalid() {
                self.stdio[i] = Some(duplicate.0 as usize);
            }
            result.map_err(|cause| RegistryError::new("fixture_output_duplicate_role", cause))?;
            let raw = self.stdio[i]
                .ok_or_else(|| RegistryError::invalid("fixture_output_duplicate_missing"))?;
            if self.handles[..i].contains(&raw) {
                return Err(RegistryError::invalid("fixture_output_duplicate_alias"));
            }
            self.handles[i] = raw;
        }
        Ok(())
    }
}
fn validate_role(index: usize, flags: u32, kind: u32) -> Result<(), RegistryError> {
    if index >= 3
        || flags & HANDLE_FLAG_INHERIT.0 == 0
        || (index == 0 && kind != FILE_TYPE_CHAR.0)
        || (index != 0 && kind != FILE_TYPE_PIPE.0)
    {
        return Err(RegistryError::invalid("fixture_output_inherited_role"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_forwarding_rejects_foreign_type_missing_inherit_and_wrong_role()
    -> Result<(), RegistryError> {
        for (i, kind) in [(0, FILE_TYPE_CHAR.0), (1, FILE_TYPE_PIPE.0), (2, FILE_TYPE_PIPE.0)] {
            validate_role(i, HANDLE_FLAG_INHERIT.0, kind)?;
            assert!(validate_role(i, 0, kind).is_err());
        }
        assert!(validate_role(0, HANDLE_FLAG_INHERIT.0, FILE_TYPE_PIPE.0).is_err());
        assert!(validate_role(1, HANDLE_FLAG_INHERIT.0, FILE_TYPE_CHAR.0).is_err());
        assert!(validate_role(3, HANDLE_FLAG_INHERIT.0, FILE_TYPE_PIPE.0).is_err());
        Ok(())
    }
}
