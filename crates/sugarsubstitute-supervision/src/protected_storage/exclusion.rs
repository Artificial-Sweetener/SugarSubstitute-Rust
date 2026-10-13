// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain stable exclusion files independently of each owner's record policy.
use super::{
    native_file::NativeCauseDestinations,
    native_name::{NativeName, NativeSid},
    native_phase::NativePhase,
    paths,
    record_file::Stage,
    security,
};
#[cfg(test)]
pub(crate) mod fixture;
use crate::{
    ClaimError,
    native_claims::{
        files::{Budget, Handle, close_all},
        lock,
    },
};
#[cfg(test)]
use std::path::Path;
use std::{path::PathBuf, sync::Mutex};
use windows::{
    Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        Storage::FileSystem::*,
    },
    core::PCWSTR,
};

pub(crate) struct HeldFile<const N: usize = 128> {
    pub(crate) handle: Handle,
    pub(crate) record_path: NativeName,
    exclusion_path: NativeName,
    locked: bool,
    pub(crate) created: bool,
    pub(crate) account_sid: NativeSid,
    causes: NativeCauseDestinations,
    pub(crate) temporary_handles: Mutex<Vec<Handle>>,
    pub(crate) stage: Mutex<Option<Stage<N>>>,
    pub(crate) publication: Mutex<Option<super::record_file::PublicationBinding<N>>>,
    #[cfg(test)]
    fixture_receipt: Option<fixture::FixtureReceipt>,
}
impl<const N: usize> HeldFile<N> {
    pub(crate) fn is_locked(&self) -> bool {
        self.locked
    }

    pub(crate) fn acquire_into(
        retained: &mut Option<Self>,
        path: NativeName,
        record_path: NativeName,
        sid: NativeSid,
        budget: Budget,
        phase: &mut NativePhase<'_>,
    ) -> Result<(), ClaimError> {
        phase.check_budget(&budget)?;
        if retained.is_some() || !(16..=1024).contains(&N) {
            return Err(phase.invalid("exclusion_storage_state"));
        }
        let causes = match &budget {
            Budget::Diagnostic(original) => {
                if !path.matches_original(original)
                    || !record_path.matches_original(original)
                    || !sid.matches_original(original)
                {
                    return Err(budget.rejection("exclusion_changed_origin"));
                }
                NativeCauseDestinations::diagnostic(original)?
            }
            _ if path.is_ordinary() && record_path.is_ordinary() && sid.is_ordinary() => {
                NativeCauseDestinations::ordinary()
            }
            _ => return Err(budget.rejection("exclusion_ordinary_origin")),
        };
        *retained = Some(Self {
            handle: Handle::new(windows::Win32::Foundation::HANDLE::default(), budget),
            record_path,
            exclusion_path: path,
            locked: false,
            created: false,
            account_sid: sid,
            causes,
            temporary_handles: Mutex::new(Vec::new()),
            stage: Mutex::new(None),
            publication: Mutex::new(None),
            #[cfg(test)]
            fixture_receipt: None,
        });
        let held = retained.as_mut().ok_or_else(|| phase.invalid("missing_claim"))?;
        let result = held.acquire(phase);
        phase.post_current_for(result, &mut held.causes)
    }

    fn acquire(&mut self, phase: &mut NativePhase<'_>) -> Result<(), ClaimError> {
        let mut phase = phase.for_causes(&mut self.causes)?;
        let producing =
            phase.open_buffers(self.exclusion_path.path()?, Some(self.account_sid.bytes()?))?;
        let attributes = producing.attributes();
        producing.check()?;
        // SAFETY: The bounded NUL path and noninheritable protected descriptor
        // live through OPEN_ALWAYS; invalid-return handles are not owned outputs.
        let raw = unsafe {
            CreateFileW(
                PCWSTR(producing.units().as_ptr()),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                attributes.as_ref().map(std::ptr::from_ref),
                OPEN_ALWAYS,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        };
        // SAFETY: Read the current thread's creation disposition immediately.
        let created = raw.is_ok() && unsafe { GetLastError() } != ERROR_ALREADY_EXISTS;
        if let Ok(raw) = &raw {
            self.handle.adopt_output(*raw);
        }
        self.created = created;
        drop(producing);
        raw.map_err(|cause| {
            phase.error("open_claim", crate::claim_error::BoundedNativeCause::Windows(cause))
        })?;
        paths::validate_regular(&self.handle, Some(0), &mut phase)?;
        security::verify(&self.handle, self.account_sid.bytes()?, &mut phase)?;
        phase.check()?;
        self.handle.borrow_file()?.try_lock().map_err(|cause| {
            phase.error("claim_exclusion", crate::claim_error::BoundedNativeCause::Lock(cause))
        })?;
        self.locked = true;
        Ok(())
    }

    /// Observe actual output custody without treating a lock flag as native closure.
    pub(crate) fn has_output(&self) -> bool {
        self.handle.is_armed()
            || lock(&self.temporary_handles).iter().any(Handle::is_armed)
            || lock(&self.stage).as_ref().is_some_and(|stage| stage.handle.is_armed())
    }

    /// Settle the existing writer exclusion under genuine original storage cleanup.
    /// Every fresh unlock/close is gated independently; successful late returns are
    /// adopted without another provider call on the correlated return path.
    pub(crate) fn retire_diagnostic(
        &mut self,
        retirement: &mut crate::diagnostic_storage::DiagnosticNativeRetirement<'_>,
    ) -> Result<(), ClaimError> {
        let Budget::Diagnostic(original) = self.handle.budget() else {
            return Err(self.handle.budget().rejection("exclusion_retirement_origin"));
        };
        if !lock(&self.temporary_handles).is_empty()
            || lock(&self.stage).is_some()
            || lock(&self.publication).is_some()
        {
            return Err(ClaimError::diagnostic_allocation(original.reject_native_state()));
        }
        if self.locked {
            self.causes.check(true)?;
            let observed = retirement.check(&original).map_err(ClaimError::diagnostic_allocation);
            self.causes.retirement_control(observed)?;
            self.handle.borrow_file()?.unlock().map_err(|cause| {
                self.causes.error(
                    "unlock_claim",
                    crate::claim_error::BoundedNativeCause::RawIo(cause),
                    true,
                )
            })?;
            self.locked = false;
        }
        self.handle.close_diagnostic(retirement, &mut self.causes)
    }

    pub(crate) fn close(&mut self) -> Result<(), ClaimError> {
        if !self.handle.budget().is_ordinary() {
            return Err(self.handle.budget().rejection("exclusion_requires_original_retirement"));
        }
        if self.locked && self.handle.raw().0.is_null() {
            return Err(ClaimError::invalid("missing_locked_exclusion_handle"));
        }
        close_all(&mut lock(&self.temporary_handles))?;
        if let Some(stage) = lock(&self.stage).as_mut() {
            stage.handle.close()?;
        }
        if self.locked {
            let result = self.handle.borrow_file()?.unlock();
            result.map_err(|e| ClaimError::new("unlock_claim", e))?;
            self.locked = false;
        }
        self.handle.close()
    }
}
impl<const N: usize> Drop for HeldFile<N> {
    fn drop(&mut self) {
        if !self.handle.budget().is_ordinary() {
            if self.has_output() {
                let empty = Self {
                    handle: Handle::new(
                        windows::Win32::Foundation::HANDLE::default(),
                        self.handle.budget(),
                    ),
                    record_path: NativeName::Ordinary(PathBuf::new()),
                    exclusion_path: NativeName::Ordinary(PathBuf::new()),
                    locked: false,
                    created: false,
                    account_sid: NativeSid::Ordinary(Vec::new()),
                    causes: NativeCauseDestinations::ordinary(),
                    temporary_handles: Mutex::new(Vec::new()),
                    stage: Mutex::new(None),
                    publication: Mutex::new(None),
                    #[cfg(test)]
                    fixture_receipt: None,
                };
                std::mem::forget(std::mem::replace(self, empty));
                tracing::error!(
                    operation = "exclusion_abandoned",
                    "Native exclusion custody retained"
                );
            }
            return;
        }
        if let Err(cause) = self.close() {
            tracing::error!(operation = cause.operation(), "Native exclusion closure failed");
        }
    }
}
