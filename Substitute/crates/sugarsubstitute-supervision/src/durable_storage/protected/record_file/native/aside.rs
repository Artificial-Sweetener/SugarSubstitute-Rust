// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rename an unreadable protected record beside its target without rewriting its bytes.
use super::*;
use crate::durable_storage::protected::native_rename;
use std::path::PathBuf;

/// Move the exact protected target to a fresh sibling name, keeping its inode and bytes.
///
/// The caller holds every exclusion that guards the target. The opened source is adopted into
/// the file's temporary handles before validation, so a failed close stays with its owner.
/// # Errors
/// Preserves context, open, regular-file, security, path, randomness and rename failures; an
/// absent target reports `record_aside_missing` without creating anything.
pub(in crate::durable_storage::protected::record_file) fn set_aside<const N: usize>(
    file: &HeldFile<N>,
    check: &mut dyn FnMut() -> Result<(), ClaimError>,
    operation: &crate::durable_storage::protected::exclusion::operation::Operation<'_>,
) -> Result<PathBuf, ClaimError> {
    let record = file.record_path.path()?.to_owned();
    let wide = paths::wide(&record)?;
    let mut handles = crate::durable_storage::protected::exclusion::operation::Loan::take(
        &file.temporary_handles,
        operation,
    );
    if handles.len() >= 16 {
        return Err(ClaimError::invalid("temporary_handle_bound"));
    }
    handles
        .try_reserve_exact(1)
        .map_err(|cause| ClaimError::new("record_handle_reservation", cause))?;
    check()?;
    // SAFETY: The bounded owned NUL path remains live through OPEN_EXISTING; the output is
    // adopted into retained custody before any validation can fail.
    let raw = match unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_READ.0 | DELETE.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    } {
        Ok(raw) => raw,
        Err(cause) if cause.code() == ERROR_FILE_NOT_FOUND.to_hresult() => {
            return Err(ClaimError::invalid("record_aside_missing"));
        }
        Err(cause) => return Err(ClaimError::new("open_record_aside", cause)),
    };
    let index = handles.len();
    handles.push(Handle::new(raw, file.handle.budget()));
    let handle = &mut handles[index];
    let moved = (|| {
        check()?;
        paths::validate_regular(
            handle,
            None,
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &handle.budget(),
            )?,
        )?;
        security::verify(
            handle,
            file.account_sid.bytes()?,
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &handle.budget(),
            )?,
        )?;
        if paths::final_path(
            handle,
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &handle.budget(),
            )?,
            crate::durable_storage::protected::native_name::NativeNameDestination::Ordinary,
        )
        .and_then(crate::durable_storage::protected::native_name::NativeName::into_ordinary)?
            != record
        {
            return Err(ClaimError::invalid("changed_record_path"));
        }
        let nonce =
            random(&mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &handle.budget(),
            )?)?;
        let aside = record.with_extension(format!("record.{}.unreadable", hex(&nonce)));
        check()?;
        native_rename::rename(
            handle,
            native_rename::Target::SameDirectory(&aside),
            false,
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &handle.budget(),
            )?,
        )?;
        Ok(aside)
    })();
    let cleanup = handle.close();
    if cleanup.is_ok() {
        handles.remove(index);
    }
    ClaimError::finish(moved, cleanup)
}
