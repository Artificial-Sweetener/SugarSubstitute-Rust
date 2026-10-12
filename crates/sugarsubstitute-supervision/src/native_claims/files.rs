// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep stable exclusion inodes and observe unlock/handle closure separately.
use super::*;
use std::{
    fs::File,
    os::windows::io::{FromRawHandle, IntoRawHandle},
    sync::Arc,
};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Storage::FileSystem::*,
};
use windows::core::PCWSTR;

#[derive(Clone)]
pub(crate) enum Budget {
    Core(Arc<crate::claims::pool::Pool>),
    Slot(Arc<SlotUse>),
    Protected(Arc<sugarsubstitute_contracts::execution::MemoryClaim>),
}
impl Budget {
    pub(crate) fn retain_failed(&self) {
        match self {
            Self::Core(value) => std::mem::forget(value.clone()),
            Self::Slot(value) => std::mem::forget(value.clone()),
            Self::Protected(value) => std::mem::forget(value.clone()),
        }
    }
}
pub(crate) struct Handle {
    value: usize,
    _budget: Budget,
}
impl Handle {
    pub(crate) fn new(value: HANDLE, budget: Budget) -> Self {
        Self { value: value.0 as usize, _budget: budget }
    }
    pub(crate) fn raw(&self) -> HANDLE {
        HANDLE(self.value as *mut _)
    }
    pub(crate) fn budget(&self) -> Budget {
        self._budget.clone()
    }
    pub(crate) fn take_file(&mut self) -> File {
        let value = std::mem::take(&mut self.value);
        // SAFETY: Unique ownership moves into File; this wrapper is disarmed first.
        unsafe { File::from_raw_handle(value as *mut _) }
    }
    pub(crate) fn put_file(&mut self, file: File) {
        self.value = file.into_raw_handle() as usize;
    }
    pub(crate) fn close(&mut self) -> Result<(), ClaimError> {
        if self.value != 0 {
            // SAFETY: The unique owner retains this valid handle until CloseHandle succeeds.
            unsafe { CloseHandle(self.raw()) }.map_err(|e| ClaimError::new("close_handle", e))?;
            self.value = 0;
        }
        Ok(())
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            tracing::error!(operation = cause.operation(), "Native claim handle closure failed");
            // A failed close retains precisely this allocation's original admitted charge.
            self._budget.retain_failed();
        }
    }
}
pub(crate) fn close_all(handles: &mut Vec<Handle>) -> Result<(), ClaimError> {
    for handle in handles.iter_mut().rev() {
        handle.close()?;
    }
    handles.clear();
    Ok(())
}

pub(crate) struct HeldFile {
    pub(crate) handle: Handle,
    pub(crate) record_path: PathBuf,
    locked: bool,
    pub(crate) created: bool,
    pub(crate) account_sid: Vec<u8>,
    pub(crate) temporary_handles: Mutex<Vec<Handle>>,
    pub(crate) stage: Mutex<Option<records::Stage>>,
}
impl HeldFile {
    pub(crate) fn is_locked(&self) -> bool {
        self.locked
    }
    pub(crate) fn acquire_into(
        retained: &mut Option<Self>,
        path: &Path,
        record_path: PathBuf,
        sid: &[u8],
        budget: Budget,
    ) -> Result<(), ClaimError> {
        let wide = paths::wide(path)?;
        let descriptor = security::Descriptor::new(sid)?;
        let attributes = descriptor.attributes();
        // SAFETY: The NUL-terminated path and noninheritable descriptor live through CreateFile.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                Some(&attributes),
                OPEN_ALWAYS,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|e| ClaimError::new("open_claim", e));
        let raw = raw?;
        // SAFETY: Read this thread's last-error immediately after successful OPEN_ALWAYS.
        let created = unsafe { windows::Win32::Foundation::GetLastError() }
            != windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
        *retained = Some(Self {
            handle: Handle::new(raw, budget),
            record_path,
            locked: false,
            created,
            account_sid: sid.to_vec(),
            temporary_handles: Mutex::new(Vec::new()),
            stage: Mutex::new(None),
        });
        let held = retained.as_mut().ok_or_else(|| ClaimError::invalid("missing_claim"))?;
        paths::validate_regular(&held.handle, Some(0))?;
        security::verify(&held.handle, sid)?;
        // SAFETY: File temporarily owns the unique handle; it is moved back before any return.
        let file = unsafe { File::from_raw_handle(held.handle.value as *mut _) };
        let result = file.try_lock();
        held.handle.value = file.into_raw_handle() as usize;
        result.map_err(|e| ClaimError::new("claim_exclusion", e))?;
        held.locked = true;
        Ok(())
    }
    pub(crate) fn close(&mut self) -> Result<(), ClaimError> {
        close_all(&mut lock(&self.temporary_handles))?;
        if let Some(stage) = lock(&self.stage).as_mut() {
            stage.handle.close()?;
        }
        if self.locked {
            // SAFETY: The live unique handle moves into File and is restored before error propagation.
            let file = unsafe { File::from_raw_handle(self.handle.value as *mut _) };
            let result = file.unlock();
            self.handle.value = file.into_raw_handle() as usize;
            result.map_err(|e| ClaimError::new("unlock_claim", e))?;
            self.locked = false;
        }
        self.handle.close()
    }
}
impl Drop for HeldFile {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            tracing::error!(operation = cause.operation(), "Native exclusion closure failed");
        }
    }
}
pub(crate) fn acquire_backend(backend: &mut Backend, directory: &Path) -> Result<(), ClaimError> {
    let digest = records::hex(&backend.proof.digest);
    let stem = directory.join("backends").join(digest);
    HeldFile::acquire_into(
        &mut backend.file,
        &stem.with_extension("claim"),
        stem.with_extension("record"),
        &backend.proof.account_sid,
        Budget::Slot(backend.proof.slot.clone()),
    )?;
    let file = backend.file.as_ref().ok_or_else(|| ClaimError::invalid("missing_claim"))?;
    let record = Record::backend(backend.resource, backend.proof.digest);
    records::check_predecessor(file, &backend.proof.account_sid, &record)?;
    backend.record = Some(record.clone());
    records::publish(file, &record)?;
    backend.claimed = true;
    Ok(())
}
