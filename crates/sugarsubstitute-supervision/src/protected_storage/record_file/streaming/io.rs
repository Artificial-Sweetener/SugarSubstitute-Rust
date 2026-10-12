// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt actual file outputs before checking protection and bounded streaming facts.
use super::*;
use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
impl StreamingPort {
    pub(super) fn open_owned(
        &mut self,
        path: PathBuf,
        bound: u64,
        stage: bool,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        self.open_file(path, bound, stage, stage, context)
    }
    pub(super) fn open_file(
        &mut self,
        path: PathBuf,
        bound: u64,
        stage: bool,
        writable: bool,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError> {
        self.check(context)?;
        if self.files.len() >= FILE_SLOTS || bound == 0 {
            return Err(ClaimError::invalid("diagnostic_file_capacity"));
        }
        let id = self.next;
        self.next =
            id.checked_add(1).ok_or_else(|| ClaimError::invalid("diagnostic_lease_exhausted"))?;
        let units = paths::wide(&path)?;
        let descriptor = security::Descriptor::new(&self.sid)?;
        let attributes = descriptor.attributes();
        // SAFETY: Bounded owned name and protected noninheritable descriptor remain
        // live. CREATE_NEW never overwrites; all read leases exclude writes/deletion.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(units.as_ptr()),
                FILE_GENERIC_READ.0 | DELETE.0 | if writable { FILE_GENERIC_WRITE.0 } else { 0 },
                FILE_SHARE_READ,
                stage.then_some(&attributes),
                if stage { CREATE_NEW } else { OPEN_EXISTING },
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|cause| ClaimError::new("diagnostic_open", cause))?;
        let index = self.files.len();
        self.files.push(NativeFile {
            id,
            handle: Handle::new(raw, Budget::Protected(self.memory.clone())),
            path,
            facts: None,
            bound,
            stage,
            deleted: false,
        });
        let file = &self.files[index];
        paths::validate_regular(&file.handle, None)?;
        security::verify(&file.handle, &self.sid)?;
        if paths::final_path(&file.handle)? != file.path {
            return Err(ClaimError::invalid("diagnostic_changed_path"));
        }
        let facts = self.observe(index, context)?;
        self.files[index].facts = Some(facts);
        self.check(context)?;
        Ok((id, facts))
    }
    pub(super) fn observe(
        &mut self,
        index: usize,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.check(context)?;
        let file = &mut self.files[index];
        paths::validate_regular(&file.handle, None)?;
        security::verify(&file.handle, &self.sid)?;
        let identity = paths::file_identity(&file.handle)?;
        let mut bytes = 0_i64;
        // SAFETY: Retained actual file and initialized exact output remain live.
        unsafe { GetFileSizeEx(file.handle.raw(), &mut bytes) }
            .map_err(|cause| ClaimError::new("diagnostic_size", cause))?;
        let bytes =
            u64::try_from(bytes).map_err(|cause| ClaimError::new("diagnostic_size", cause))?;
        if bytes > file.bound {
            return Err(ClaimError::invalid("diagnostic_size_bound"));
        }
        let mut hash = Sha256::new();
        let mut buffer = vec![0; DIAGNOSTIC_IO_BYTES];
        seek(&file.handle, 0)?;
        let mut remaining = bytes;
        while remaining != 0 {
            super::super::super::check(context)?;
            let count = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|cause| ClaimError::new("diagnostic_chunk", cause))?;
            read_exact(&file.handle, &mut buffer[..count])?;
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
        paths::validate_regular(&file.handle, Some(bytes as i64))?;
        if paths::file_identity(&file.handle)? != identity {
            return Err(ClaimError::invalid("diagnostic_changed_identity"));
        }
        super::super::super::check(context)?;
        Ok(DiagnosticReadFacts { identity, sha256: hash.finalize().into(), bytes })
    }
    pub(super) fn read_owned(
        &mut self,
        id: NativeId,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
    ) -> Result<usize, ClaimError> {
        self.check(context)?;
        let index = self.index(id)?;
        let file = &mut self.files[index];
        let facts = file.facts.ok_or_else(|| ClaimError::invalid("diagnostic_unvalidated_read"))?;
        if output.len() > DIAGNOSTIC_IO_BYTES
            || offset.checked_add(output.len() as u64).is_none_or(|end| end > facts.bytes)
            || file.deleted
            || paths::file_identity(&file.handle)? != facts.identity
        {
            return Err(ClaimError::invalid("diagnostic_read_range"));
        }
        seek(&file.handle, offset)?;
        read_exact(&file.handle, output)?;
        self.check(context)?;
        Ok(output.len())
    }
    pub(super) fn write_owned(
        &mut self,
        id: NativeId,
        offset: u64,
        input: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.check(context)?;
        let index = self.index(id)?;
        let file = &mut self.files[index];
        if !file.stage
            || file.deleted
            || input.len() > DIAGNOSTIC_IO_BYTES
            || offset.checked_add(input.len() as u64).is_none_or(|end| end > file.bound)
        {
            return Err(ClaimError::invalid("diagnostic_write_range"));
        }
        let original =
            file.facts.ok_or_else(|| ClaimError::invalid("diagnostic_unvalidated_stage"))?;
        if paths::file_identity(&file.handle)? != original.identity {
            return Err(ClaimError::invalid("diagnostic_changed_stage"));
        }
        seek(&file.handle, offset)?;
        let mut written = 0;
        // SAFETY: The complete caller slice is bounded above; actual synchronous
        // file custody stays armed through every status and clock observation.
        unsafe { WriteFile(file.handle.raw(), Some(input), Some(&mut written), None) }
            .map_err(|cause| ClaimError::new("diagnostic_write", cause))?;
        if written as usize != input.len() {
            return Err(ClaimError::invalid("diagnostic_short_write"));
        }
        self.check(context)
    }
    pub(super) fn flush_owned(
        &mut self,
        id: NativeId,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError> {
        self.check(context)?;
        let index = self.index(id)?;
        let file = &mut self.files[index];
        // SAFETY: The actual retained writable file remains armed through flush.
        unsafe { FlushFileBuffers(file.handle.raw()) }
            .map_err(|cause| ClaimError::new("diagnostic_flush", cause))?;
        let actual = self.observe(index, context)?;
        if self.files[index].facts.is_some_and(|prior| prior.identity != actual.identity) {
            return Err(ClaimError::invalid("diagnostic_changed_stage"));
        }
        self.files[index].facts = Some(actual);
        Ok(actual)
    }
}
fn seek(handle: &Handle, offset: u64) -> Result<(), ClaimError> {
    let offset =
        i64::try_from(offset).map_err(|cause| ClaimError::new("diagnostic_offset", cause))?;
    // SAFETY: The private synchronous file has exclusive owner access under the
    // storage mutex; its actual handle remains retained throughout the seek.
    unsafe { SetFilePointerEx(handle.raw(), offset, None, FILE_BEGIN) }
        .map_err(|cause| ClaimError::new("diagnostic_seek", cause))
}
fn read_exact(handle: &Handle, output: &mut [u8]) -> Result<(), ClaimError> {
    let mut read = 0;
    // SAFETY: Owned initialized output is at most64KiB and exact length is
    // advertised. Synchronous read leaves native custody with its original owner.
    unsafe { ReadFile(handle.raw(), Some(output), Some(&mut read), None) }
        .map_err(|cause| ClaimError::new("diagnostic_read", cause))?;
    if read as usize != output.len() {
        return Err(ClaimError::invalid("diagnostic_short_read"));
    }
    Ok(())
}
