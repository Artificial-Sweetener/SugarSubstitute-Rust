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
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &file.handle.budget(),
            )?,
        )?;
        security::verify(
            &file.handle,
            file.account_sid.bytes()?,
            &mut crate::durable_storage::protected::native_phase::NativePhase::ordinary(
                &file.handle.budget(),
            )?,
        )
    }
    fn read_shape(
        &mut self,
        file: &HeldFile<N>,
        expected: Option<[u8; 24]>,
        check: &mut dyn FnMut() -> Result<(), ClaimError>,
        operation: &crate::durable_storage::protected::exclusion::operation::Operation<'_>,
    ) -> Result<RecordRead<N>, ClaimError> {
        read::shape(file, expected, check, operation)
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
        value.handle.budget().check()?;
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
        operation: &crate::durable_storage::protected::exclusion::operation::Operation<'_>,
    ) -> Result<(), ClaimError> {
        let mut handles = crate::durable_storage::protected::exclusion::operation::Loan::take(
            &file.temporary_handles,
            operation,
        );
        for handle in handles.iter_mut().rev() {
            check()?;
            let result = handle.close();
            ClaimError::finish(result, check())?;
        }
        handles.clear();
        Ok(())
    }
}
/// Observe a checked nonzero cryptographic nonce for an owned staging name.
/// # Errors
/// Return the original native RNG failure or reject an all-zero nonce.
pub(crate) fn random(
    phase: &mut crate::durable_storage::protected::native_phase::NativePhase<'_>,
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
