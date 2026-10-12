// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep stable exclusion inodes and observe unlock/handle closure separately.
use super::*;
use std::{
    fs::File,
    os::windows::io::{FromRawHandle, IntoRawHandle},
    sync::Arc,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE};

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

pub(crate) fn acquire_backend(
    backend: &mut Backend,
    directory: &Path,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> Result<(), ClaimError> {
    let digest = crate::protected_storage::record_file::hex(&backend.proof.digest);
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
    backend.original_released =
        records::check_predecessor(file, &backend.proof.account_sid, &record, context)?;
    backend.record = Some(record.clone());
    records::publish(file, &record, context)?;
    backend.claimed = true;
    Ok(())
}
