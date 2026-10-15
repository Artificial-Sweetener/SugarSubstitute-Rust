// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt writable stage outputs before every native validation or activation check.
use super::*;
pub(super) fn create<const N: usize>(
    file: &HeldFile<N>,
    value: &mut Stage<N>,
) -> Result<(), ClaimError> {
    value.path = file.record_path.path()?.with_extension(format!(
        "{}.stage",
        hex(&random(&mut crate::protected_storage::native_phase::NativePhase::ordinary(
            &file.handle.budget()
        )?)?)
    ));
    let wide = paths::wide(&value.path)?;
    let descriptor = security::Descriptor::new(
        file.account_sid.bytes()?,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&file.handle.budget())?,
    )?;
    let attributes = descriptor.attributes();
    file.handle.budget().check()?;
    // SAFETY: Bounded owned name and protected noninheritable descriptor remain live.
    let raw = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | DELETE.0,
            FILE_SHARE_READ,
            Some(&attributes),
            CREATE_NEW,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|cause| ClaimError::new("stage_claim_record", cause))?;
    value.handle = Handle::new(raw, file.handle.budget());
    Ok(())
}
pub(super) fn reopen<const N: usize>(
    file: &HeldFile<N>,
    value: &mut Stage<N>,
) -> Result<(), ClaimError> {
    let wide = paths::wide(&value.path)?;
    file.handle.budget().check()?;
    // SAFETY: Only the exact retained stage path is opened; output is adopted before checks.
    let raw = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | DELETE.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|cause| ClaimError::new("reopen_claim_stage", cause))?;
    value.handle = Handle::new(raw, file.handle.budget());
    Ok(())
}
pub(super) fn verify<const N: usize>(
    file: &HeldFile<N>,
    value: &mut Stage<N>,
) -> Result<(), ClaimError> {
    paths::validate_regular(
        &value.handle,
        None,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&value.handle.budget())?,
    )?;
    security::verify(
        &value.handle,
        file.account_sid.bytes()?,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&value.handle.budget())?,
    )?;
    let identity = paths::file_identity(
        &value.handle,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&value.handle.budget())?,
    )?;
    if value.identity.is_some_and(|previous| previous != identity)
        || paths::final_path(
            &value.handle,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &value.handle.budget(),
            )?,
            crate::protected_storage::native_name::NativeNameDestination::Ordinary,
        )
        .and_then(crate::protected_storage::native_name::NativeName::into_ordinary)?
            != value.path
    {
        return Err(ClaimError::invalid("changed_claim_stage"));
    }
    value.identity = Some(identity);
    Ok(())
}
pub(super) fn write<const N: usize>(
    value: &mut Stage<N>,
    encoded: &[u8; N],
    check: &mut dyn FnMut() -> Result<(), ClaimError>,
) -> Result<(), ClaimError> {
    let mut written = 0;
    check()?;
    // SAFETY: Retained writable stage stays armed through bounded synchronous IO.
    unsafe {
        SetFilePointerEx(value.handle.raw(), 0, None, FILE_BEGIN)
            .map_err(|cause| ClaimError::new("seek_claim_record", cause))?;
        check()?;
        SetEndOfFile(value.handle.raw())
            .map_err(|cause| ClaimError::new("truncate_claim_record", cause))?;
        check()?;
        WriteFile(value.handle.raw(), Some(encoded), Some(&mut written), None)
            .map_err(|cause| ClaimError::new("write_claim_record", cause))?;
        check()?;
        if written as usize != N {
            return Err(ClaimError::invalid("short_claim_write"));
        }
        check()?;
        FlushFileBuffers(value.handle.raw())
            .map_err(|cause| ClaimError::new("flush_claim_record", cause))?;
    }
    check()
}
pub(super) fn activate<const N: usize>(
    file: &HeldFile<N>,
    value: &mut Stage<N>,
) -> Result<(), ClaimError> {
    let target = file.record_path.path()?.to_owned();
    rename(
        &value.handle,
        RenameTarget::Absolute(&target),
        true,
        &mut crate::protected_storage::native_phase::NativePhase::ordinary(&value.handle.budget())?,
    )?;
    value.path = target;
    value.activated = Some(value.encoded);
    Ok(())
}
