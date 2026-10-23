// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the sole actual size/hash/identity and bounded read algorithms.
use super::*;
use crate::protected_storage::native_phase::NativePhase;
use ring::digest::{Context, SHA256};
use windows::Win32::Storage::FileSystem::*;
impl NativeFile {
    /// Stream whole-file SHA256 under retained native and original context custody.
    /// # Errors
    /// Size, identity, account, short read and cancellation errors retain the capsule.
    pub(crate) fn observe(
        &mut self,
        sid: Option<&[u8]>,
        context: &JobContext,
        phase: &mut NativePhase<'_>,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        let result = self.observe_once(sid, context, phase);
        let result = phase.post_context_for(result, context, &mut self.fields_mut()?.causes);
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    fn observe_once(
        &mut self,
        sid: Option<&[u8]>,
        context: &JobContext,
        phase: &mut NativePhase<'_>,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.check(context)?;
        phase.check_budget(&self.fields()?.handle.budget())?;
        let Fields { handle, bound, causes, .. } = self.fields_mut()?;
        let mut phase = phase.for_causes(causes)?;
        phase.context(context)?;
        paths::validate_regular(handle, None, &mut phase)?;
        if let Some(sid) = sid {
            security::verify(handle, sid, &mut phase)?;
        }
        let identity = paths::file_identity(handle, &mut phase)?;
        let mut bytes = 0_i64;
        phase.context(context)?;
        // SAFETY: Retained actual file and initialized exact output remain live.
        unsafe { GetFileSizeEx(handle.raw(), &mut bytes) }
            .map_err(|cause| phase.error("diagnostic_size", BoundedNativeCause::Windows(cause)))?;
        let bytes = u64::try_from(bytes)
            .map_err(|cause| phase.error("diagnostic_size", BoundedNativeCause::Integer(cause)))?;
        if bytes > *bound {
            return Err(phase.invalid("diagnostic_size_bound"));
        }
        let hash_allocation = phase.hash_destination()?;
        let mut hash = Context::new(&SHA256);
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(DIAGNOSTIC_IO_BYTES).map_err(|cause| {
            phase.error("diagnostic_read_buffer", BoundedNativeCause::Reserve(cause))
        })?;
        if buffer.capacity() > DIAGNOSTIC_IO_BYTES {
            return Err(phase.invalid("diagnostic_read_buffer_capacity"));
        }
        buffer.resize(DIAGNOSTIC_IO_BYTES, 0);
        seek(handle, 0, &mut phase)?;
        let mut remaining = bytes;
        while remaining != 0 {
            phase.context(context)?;
            let count = usize::try_from(remaining.min(buffer.len() as u64)).map_err(|cause| {
                phase.error("diagnostic_chunk", BoundedNativeCause::Integer(cause))
            })?;
            read_exact(handle, &mut buffer[..count], &mut phase)?;
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
        drop(buffer);
        let mut sha256 = [0; 32];
        sha256.copy_from_slice(hash.finish().as_ref());
        drop(hash_allocation);
        paths::validate_regular(handle, Some(bytes as i64), &mut phase)?;
        if paths::file_identity(handle, &mut phase)? != identity {
            return Err(phase.invalid("diagnostic_changed_identity"));
        }
        phase.context(context)?;
        let facts = DiagnosticReadFacts { identity, sha256, bytes };
        Ok(facts)
    }
    /// Read at most64KiB from the same fully observed pinned file.
    /// # Errors
    /// Rejects changed native identity, retired/deleted files and range/context errors.
    pub(crate) fn read(
        &mut self,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
        phase: &mut NativePhase<'_>,
    ) -> Result<usize, ClaimError> {
        let result = self.read_once(offset, output, context, phase);
        let result = phase.post_context_for(result, context, &mut self.fields_mut()?.causes);
        if let Err(cause) = &result {
            self.remember(cause.clone());
        }
        result
    }
    fn read_once(
        &mut self,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
        phase: &mut NativePhase<'_>,
    ) -> Result<usize, ClaimError> {
        self.check(context)?;
        phase.check_budget(&self.fields()?.handle.budget())?;
        let Fields { handle, facts, deleted, causes, .. } = self.fields_mut()?;
        let mut phase = phase.for_causes(causes)?;
        phase.context(context)?;
        let facts = (*facts).ok_or_else(|| phase.invalid("diagnostic_unvalidated_read"))?;
        if output.len() > DIAGNOSTIC_IO_BYTES
            || offset.checked_add(output.len() as u64).is_none_or(|end| end > facts.bytes)
            || *deleted
            || paths::file_identity(handle, &mut phase)? != facts.identity
        {
            return Err(phase.invalid("diagnostic_read_range"));
        }
        seek(handle, offset, &mut phase)?;
        phase.context(context)?;
        read_exact(handle, output, &mut phase)?;
        phase.context(context)?;
        Ok(output.len())
    }
}
pub(crate) fn seek(
    handle: &Handle,
    offset: u64,
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    phase.check_budget(&handle.budget())?;
    phase.check()?;
    let offset = i64::try_from(offset)
        .map_err(|cause| phase.error("diagnostic_offset", BoundedNativeCause::Integer(cause)))?;
    // SAFETY: Synchronous owner access retains the actual output throughout seeking.
    unsafe { SetFilePointerEx(handle.raw(), offset, None, FILE_BEGIN) }
        .map_err(|cause| phase.error("diagnostic_seek", BoundedNativeCause::Windows(cause)))
}
fn read_exact(
    handle: &Handle,
    output: &mut [u8],
    phase: &mut NativePhase<'_>,
) -> Result<(), ClaimError> {
    phase.check_budget(&handle.budget())?;
    phase.check()?;
    let mut read = 0;
    // SAFETY: Borrowed initialized bounded output and retained synchronous file.
    unsafe { ReadFile(handle.raw(), Some(output), Some(&mut read), None) }
        .map_err(|cause| phase.error("diagnostic_read", BoundedNativeCause::Windows(cause)))?;
    if read as usize != output.len() {
        return Err(phase.invalid("diagnostic_short_read"));
    }
    Ok(())
}
