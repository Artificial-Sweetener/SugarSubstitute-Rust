// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain stable exclusion files independently of each owner's record policy.
use super::{paths, record_file::Stage, security};
use crate::{
    ClaimError,
    native_claims::{
        files::{Budget, Handle, close_all},
        lock,
    },
};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use windows::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        Storage::FileSystem::*,
    },
    core::PCWSTR,
};

pub(crate) struct HeldFile<const N: usize = 128> {
    pub(crate) handle: Handle,
    pub(crate) record_path: PathBuf,
    locked: bool,
    pub(crate) created: bool,
    pub(crate) account_sid: Vec<u8>,
    pub(crate) temporary_handles: Mutex<Vec<Handle>>,
    pub(crate) stage: Mutex<Option<Stage<N>>>,
}
impl<const N: usize> HeldFile<N> {
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
        if retained.is_some() || !(16..=1024).contains(&N) {
            return Err(ClaimError::invalid("exclusion_storage_state"));
        }
        let units = paths::wide(path)?;
        let descriptor = security::Descriptor::new(sid)?;
        let attributes = descriptor.attributes();
        // All fallible allocation precedes the native output. The durable slot
        // exists even when later object validation or lock acquisition fails.
        let mut account_sid = Vec::new();
        account_sid
            .try_reserve_exact(sid.len())
            .map_err(|e| ClaimError::new("exclusion_sid", e))?;
        account_sid.extend_from_slice(sid);
        *retained = Some(Self {
            handle: Handle::new(windows::Win32::Foundation::HANDLE::default(), budget),
            record_path,
            locked: false,
            created: false,
            account_sid,
            temporary_handles: Mutex::new(Vec::new()),
            stage: Mutex::new(None),
        });
        let held = retained.as_mut().ok_or_else(|| ClaimError::invalid("missing_claim"))?;
        // SAFETY: The bounded NUL path and noninheritable protected descriptor
        // live through OPEN_ALWAYS; invalid-return handles are not owned outputs.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(units.as_ptr()),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                Some(&attributes),
                OPEN_ALWAYS,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|e| ClaimError::new("open_claim", e))?;
        // SAFETY: Read the current thread's creation disposition immediately.
        let created = unsafe { GetLastError() } != ERROR_ALREADY_EXISTS;
        held.handle = Handle::new(raw, held.handle.budget());
        held.created = created;
        paths::validate_regular(&held.handle, Some(0))?;
        security::verify(&held.handle, sid)?;
        let file = held.handle.take_file();
        let result = file.try_lock();
        held.handle.put_file(file);
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
            let file = self.handle.take_file();
            let result = file.unlock();
            self.handle.put_file(file);
            result.map_err(|e| ClaimError::new("unlock_claim", e))?;
            self.locked = false;
        }
        self.handle.close()
    }
}
impl<const N: usize> Drop for HeldFile<N> {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            tracing::error!(operation = cause.operation(), "Native exclusion closure failed");
        }
    }
}
