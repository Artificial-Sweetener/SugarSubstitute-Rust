// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Implement fixed-record primitives with adopted Windows handles and bounded bytes.
mod read;
mod stage;
use super::*;
use windows::{
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, Security::Cryptography::*, Storage::FileSystem::*},
    core::PCWSTR,
};
/// Adopt actual Windows outputs before implementing each finite file primitive.
pub(super) struct NativeRecordBoundary;
impl<const N: usize> RecordBoundary<N> for NativeRecordBoundary {
    fn verify_exclusion(&mut self, file: &HeldFile<N>) -> Result<(), ClaimError> {
        paths::validate_regular(
            &file.handle,
            Some(0),
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &file.handle.budget(),
            )?,
        )?;
        security::verify(
            &file.handle,
            file.account_sid.bytes()?,
            &mut crate::protected_storage::native_phase::NativePhase::ordinary(
                &file.handle.budget(),
            )?,
        )
    }
    fn read_shape(
        &mut self,
        file: &HeldFile<N>,
        expected: Option<[u8; 24]>,
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
    ) -> Result<RecordRead<N>, ClaimError> {
        read::shape(file, expected, check)
    }
    fn create_stage_into(
        &mut self,
        file: &HeldFile<N>,
        value: &mut Stage<N>,
    ) -> Result<(), ClaimError> {
        stage::create(file, value)
    }
    fn reopen_stage_into(
        &mut self,
        file: &HeldFile<N>,
        value: &mut Stage<N>,
    ) -> Result<(), ClaimError> {
        stage::reopen(file, value)
    }
    fn stage_is_open(&self, value: &Stage<N>) -> bool {
        !value.handle.raw().0.is_null()
    }
    fn verify_stage(&mut self, file: &HeldFile<N>, value: &mut Stage<N>) -> Result<(), ClaimError> {
        stage::verify(file, value)
    }
    fn write_stage(
        &mut self,
        value: &mut Stage<N>,
        encoded: &[u8; N],
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        stage::write(value, encoded, check)
    }
    fn activate_stage(
        &mut self,
        file: &HeldFile<N>,
        value: &mut Stage<N>,
    ) -> Result<(), ClaimError> {
        stage::activate(file, value)
    }
    fn read_activated(
        &mut self,
        value: &mut Stage<N>,
        expected: &[u8; N],
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        read::activated(&value.handle, expected, check)
    }
    fn flush_stage(&mut self, value: &mut Stage<N>) -> Result<(), ClaimError> {
        // SAFETY: The actual retained writable owner remains armed through flush.
        unsafe { FlushFileBuffers(value.handle.raw()) }
            .map_err(|cause| ClaimError::new("flush_activated_claim", cause))
    }
    fn close_stage(&mut self, value: &mut Stage<N>) -> Result<(), ClaimError> {
        value.handle.close()
    }
    fn close_temporary(
        &mut self,
        file: &HeldFile<N>,
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
    ) -> Result<(), ClaimError> {
        let mut handles = lock(&file.temporary_handles);
        for handle in handles.iter_mut().rev() {
            check()?;
            let result = handle.close();
            ClaimError::finish(result, check())?;
        }
        handles.clear();
        Ok(())
    }
}
/// Rename through the retained source handle to a bounded owned UTF16 target.
/// # Errors
/// Preserve source custody on invalid target shape or native activation failure.
pub(crate) fn rename(
    handle: &Handle,
    target: &std::path::Path,
    replace: bool,
    phase: &mut crate::protected_storage::native_phase::NativePhase<'_>,
) -> Result<(), ClaimError> {
    use crate::claim_error::BoundedNativeCause;
    phase.check_budget(&handle.budget())?;
    let mut producing = phase.wide(target)?;
    let units = producing.units().len() - 1;
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
    // already bounded UTF16 destination. The stage's actual handle stays retained.
    let result = unsafe {
        let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        (*information).Anonymous.ReplaceIfExists = replace;
        (*information).FileNameLength = (units * 2) as u32;
        std::ptr::copy_nonoverlapping(
            producing.units().as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast(),
            producing.units().len(),
        );
        SetFileInformationByHandle(
            handle.raw(),
            FileRenameInfo,
            buffer.as_ptr().cast(),
            bytes as u32,
        )
    };
    drop(buffer);
    drop(producing);
    // The caller adopts the actual new path before its unconditional post-control
    // observation. A late return must not leave the retained owner at the old path.
    result.map_err(|cause| phase.error("activate_claim_record", BoundedNativeCause::Windows(cause)))
}
/// Observe a checked nonzero cryptographic nonce for an owned staging name.
/// # Errors
/// Return the original native RNG failure or reject an all-zero nonce.
pub(crate) fn random(
    phase: &mut crate::protected_storage::native_phase::NativePhase<'_>,
) -> Result<[u8; 16], ClaimError> {
    phase.check()?;
    let mut bytes = [0; 16];
    // SAFETY: The exact initialized writable slice belongs to this bounded nonce operation.
    let status = unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    phase.ntstatus("claim_randomness", status)?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(phase.invalid("claim_randomness"));
    }
    Ok(bytes)
}
