// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin canonical non-reparse chains and verify content through actual retained files.
use crate::{
    CrashError,
    input::{MAX_ANCESTORS, MAX_UNITS},
};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::JobContext};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Storage::FileSystem::*,
    },
    core::PCWSTR,
};

pub(super) const MAX_HANDLES: usize = MAX_ANCESTORS * 8 + 4;
#[cfg(test)]
mod tests;
pub(super) struct Pins {
    pub(super) values: Vec<Option<usize>>,
}
pub(super) struct CloseReport {
    pub(super) first: Option<CrashError>,
    pub(super) additional: [Option<CrashError>; 8],
    pub(super) omitted: u64,
}
impl CloseReport {
    fn new() -> Self {
        Self { first: None, additional: std::array::from_fn(|_| None), omitted: 0 }
    }
    fn retain(&mut self, cause: CrashError) {
        if self.first.is_none() {
            self.first = Some(cause);
        } else if let Some(slot) = self.additional.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
}
pub(super) fn wide(path: &Path) -> Result<Vec<u16>, CrashError> {
    crate::input::validate_path(path)?;
    let mut units = Vec::new();
    units
        .try_reserve_exact(path.as_os_str().encode_wide().count() + 1)
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    units.extend(path.as_os_str().encode_wide());
    units.push(0);
    if units.capacity() > MAX_UNITS + 1 {
        return Err(CrashError::invalid(CaptureOperation::Runtime));
    }
    Ok(units)
}
fn prefix() -> Result<PathBuf, CrashError> {
    let mut path = PathBuf::new();
    path.try_reserve_exact(MAX_UNITS * 3)
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
    if path.capacity() > MAX_UNITS * 3 {
        return Err(CrashError::invalid(CaptureOperation::Runtime));
    }
    Ok(path)
}
fn canonical(units: &[u16]) -> Result<PathBuf, CrashError> {
    if units.len() > MAX_UNITS {
        return Err(CrashError::invalid(CaptureOperation::Runtime));
    }
    let path = PathBuf::from(OsString::from_wide(units));
    // Lossless standard conversion may briefly grow beyond the retained bound.
    // Its rejected 131064-byte capacity is included in the native scratch claim.
    if path.capacity() > MAX_UNITS * 3 {
        return Err(CrashError::invalid(CaptureOperation::Runtime));
    }
    crate::input::validate_path(&path)?;
    Ok(path)
}
pub(super) fn deadline(context: &JobContext) -> Result<(), CrashError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|limit| context.now() >= limit)
    {
        Err(CrashError::invalid(CaptureOperation::Runtime))
    } else {
        Ok(())
    }
}
impl Pins {
    pub(super) fn new() -> Result<Self, CrashError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(MAX_HANDLES)
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        if values.capacity() > MAX_HANDLES {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        Ok(Self { values })
    }
    pub(super) fn adopt(&mut self, raw: usize) -> Result<usize, CrashError> {
        if raw == 0
            || raw == usize::MAX
            || self.values.len() == MAX_HANDLES
            || self.values.iter().flatten().any(|old| *old == raw)
        {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        let index = self.values.len();
        self.values.push(Some(raw));
        Ok(index)
    }
    pub(super) fn reserve_outputs(&mut self) -> Result<[usize; 2], CrashError> {
        if self.values.len() > MAX_HANDLES - 2 {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        let first = self.values.len();
        self.values.extend([None, None]);
        Ok([first, first + 1])
    }
    pub(super) fn retain_outputs(
        &mut self,
        slots: [usize; 2],
        outputs: [usize; 2],
    ) -> Result<(), CrashError> {
        let mut invalid = false;
        // Every actual unique output is durably installed before any validation can return.
        for (slot, raw) in slots.into_iter().zip(outputs) {
            if raw == 0 {
                continue;
            }
            if raw == usize::MAX || self.values.iter().flatten().any(|old| *old == raw) {
                invalid = true;
            } else {
                self.values[slot] = Some(raw);
            }
        }
        if invalid { Err(CrashError::invalid(CaptureOperation::Runtime)) } else { Ok(()) }
    }
    pub(super) fn release_unused_outputs(&mut self) {
        while self.values.last() == Some(&None) {
            self.values.pop();
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.values.iter().all(Option::is_none)
    }
    pub(super) fn handle(&self, index: usize) -> Result<HANDLE, CrashError> {
        self.values
            .get(index)
            .and_then(|slot| *slot)
            .map(|raw| HANDLE(raw as *mut _))
            .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))
    }
    fn open(&mut self, path: &Path, directory: bool) -> Result<usize, CrashError> {
        if self.values.len() == MAX_HANDLES {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        let units = wide(path)?;
        // SAFETY: Owned NUL path is valid; no handle inheritance or delete sharing
        // is granted. Opening reparse points themselves prevents hidden traversal.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(units.as_ptr()),
                FILE_READ_ATTRIBUTES.0 | if directory { 0 } else { FILE_GENERIC_READ.0 },
                FILE_SHARE_READ | if directory { FILE_SHARE_WRITE } else { FILE_SHARE_MODE(0) },
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        let index = self.adopt(handle.0 as usize)?;
        let handle = self.handle(index)?;
        let mut attributes = FILE_ATTRIBUTE_TAG_INFO::default();
        // SAFETY: Actual retained handle and initialized correctly sized output.
        unsafe {
            GetFileInformationByHandleEx(
                handle,
                FileAttributeTagInfo,
                (&mut attributes as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
                size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
        }
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        if attributes.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
            || (attributes.FileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0) != directory
        {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        Ok(index)
    }
    fn identity(&self, index: usize) -> Result<FILE_ID_INFO, CrashError> {
        let mut result = FILE_ID_INFO::default();
        // SAFETY: Actual retained handle and initialized exactly sized identity output.
        unsafe {
            GetFileInformationByHandleEx(
                self.handle(index)?,
                FileIdInfo,
                (&mut result as *mut FILE_ID_INFO).cast(),
                size_of::<FILE_ID_INFO>() as u32,
            )
        }
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        Ok(result)
    }
    fn final_path(&self, index: usize) -> Result<PathBuf, CrashError> {
        let mut units = vec![0; MAX_UNITS + 1];
        // SAFETY: Fixed writable output lives through query of the retained object.
        let count = unsafe {
            GetFinalPathNameByHandleW(
                self.handle(index)?,
                &mut units,
                GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
            )
        };
        if count == 0 || count as usize >= units.len() {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        canonical(&units[..count as usize])
    }
    pub(super) fn resolve(
        &mut self,
        path: &Path,
        directory: bool,
        context: &JobContext,
    ) -> Result<(PathBuf, usize), CrashError> {
        let start = self.values.len();
        let mut prefix = prefix()?;
        let count = path.components().count();
        for (position, component) in path.components().enumerate() {
            prefix.push(component.as_os_str());
            // A bare disk prefix is not a rooted object yet.
            if position == 0 {
                continue;
            }
            deadline(context)?;
            self.open(&prefix, position + 1 != count || directory)?;
        }
        let end = self.values.len();
        if start == end {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        let target = end - 1;
        let canonical = self.final_path(target)?;
        // Reopen each actual canonical ancestor/target, compare its object ID,
        // and retain both chains before LoadLibrary/CreateProcess re-resolves names.
        for index in start..end {
            deadline(context)?;
            let canonical = self.final_path(index)?;
            let original = self.identity(index)?;
            let reopened = self.open(&canonical, index + 1 != end || directory)?;
            let actual = self.identity(reopened)?;
            if original.VolumeSerialNumber != actual.VolumeSerialNumber
                || original.FileId.Identifier != actual.FileId.Identifier
            {
                return Err(CrashError::invalid(CaptureOperation::Runtime));
            }
        }
        Ok((canonical, target))
    }
    pub(super) fn verify(
        &self,
        index: usize,
        expected: [u8; 32],
        context: &JobContext,
    ) -> Result<(), CrashError> {
        let handle = self.handle(index)?;
        let mut length = 0;
        // SAFETY: Actual pinned regular file and initialized output.
        unsafe { GetFileSizeEx(handle, &mut length) }
            .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
        if !(1..=16 * 1024 * 1024).contains(&length) {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        let mut hasher = Sha256::new();
        let mut scratch = [0; 64 * 1024];
        let mut consumed = 0i64;
        while consumed < length {
            deadline(context)?;
            let mut read = 0;
            // SAFETY: Fixed initialized byte buffer and count live through a finite
            // synchronous disk read on the admitted infrastructure worker.
            unsafe { ReadFile(handle, Some(&mut scratch), Some(&mut read), None) }
                .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))?;
            if read == 0 || read as usize > scratch.len() || consumed + i64::from(read) > length {
                return Err(CrashError::invalid(CaptureOperation::Runtime));
            }
            hasher.update(&scratch[..read as usize]);
            consumed += i64::from(read);
        }
        if <[u8; 32]>::from(hasher.finalize()) != expected {
            return Err(CrashError::invalid(CaptureOperation::Runtime));
        }
        Ok(())
    }
    pub(super) fn close(&mut self) -> CloseReport {
        let mut report = CloseReport::new();
        for slot in self.values.iter_mut().rev() {
            if let Some(raw) = *slot {
                // SAFETY: Every slot is an adopted unique owned native handle;
                // only actual successful closure removes that retained ownership.
                match unsafe { CloseHandle(HANDLE(raw as *mut _)) } {
                    Ok(()) => *slot = None,
                    Err(cause) => {
                        report.retain(CrashError::new(CaptureOperation::Retirement, cause))
                    }
                }
            }
        }
        report
    }
}
