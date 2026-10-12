// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep every returned handle in a durable slot until checked close actually succeeds.
use super::NativePipeError;
use crate::native::NativeObservationError;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::{NativeHandleClaim, NativeHandleOutput};
use windows::Win32::Foundation::*;
struct Owner {
    raw: usize,
    claim: Option<Arc<NativeHandleClaim>>,
    output: Option<NativeHandleOutput>,
    original: Option<crate::native_resources::EffectContext>,
    cause: Option<NativeObservationError>,
}
pub(crate) struct Handle {
    owner: Option<Box<Owner>>,
}
impl Handle {
    pub(crate) fn new() -> Self {
        Self {
            owner: Some(Box::new(Owner {
                raw: 0,
                claim: None,
                output: None,
                original: None,
                cause: None,
            })),
        }
    }
    pub(crate) fn attach(
        &mut self,
        handles: Arc<NativeHandleClaim>,
        original: crate::native_resources::EffectContext,
    ) {
        if let Some(owner) = &mut self.owner {
            owner.claim = Some(handles);
            owner.original = Some(original);
        }
    }
    pub(crate) fn value(&self) -> usize {
        self.owner.as_ref().map_or(0, |owner| owner.raw)
    }
    pub(crate) fn check(&self, stage: &'static str) -> Result<(), NativePipeError> {
        if let Some(original) = self.owner.as_ref().and_then(|owner| owner.original.as_ref()) {
            original.check().map_err(|cause| error(stage, cause))?;
        }
        Ok(())
    }
    pub(crate) fn reserve(&mut self, stage: &'static str) -> Result<(), NativePipeError> {
        self.check(stage)?;
        self.reserve_custody(stage)
    }
    pub(crate) fn reserve_custody(&mut self, stage: &'static str) -> Result<(), NativePipeError> {
        let owner = self
            .owner
            .as_mut()
            .ok_or_else(|| error(stage, std::io::Error::other("Native handle owner absent")))?;
        if owner.raw != 0 || owner.output.is_some() {
            return Err(error(stage, std::io::Error::other("Native output slot already retained")));
        }
        if let Some(claim) = &owner.claim {
            owner.output = Some(claim.reserve_output().map_err(|cause| error(stage, cause))?);
        }
        Ok(())
    }
    pub(crate) fn adopt_partial(&mut self, raw: HANDLE) {
        if let Some(owner) = &mut self.owner {
            if raw.is_invalid() {
                owner.output = None;
            } else {
                owner.raw = raw.0 as usize;
            }
        }
    }
    /// Retain any returned output before interpreting its ABI status or fresh authority.
    pub(crate) fn finish_output(
        &mut self,
        raw: HANDLE,
        result: windows::core::Result<()>,
        stage: &'static str,
    ) -> Result<(), NativePipeError> {
        self.adopt_partial(raw);
        if let Err(cause) = result {
            let failure = error(stage, cause);
            self.retain(&failure);
            return Err(failure);
        }
        if self.value() == 0 {
            return Err(error(stage, std::io::Error::other("Native output identity absent")));
        }
        self.check(stage)
    }
    pub(crate) fn transfer(&mut self, destination: &mut Self) {
        std::mem::swap(&mut self.owner, &mut destination.owner);
    }
    pub(crate) fn raw(&self) -> HANDLE {
        HANDLE(self.value() as *mut _)
    }
    pub(crate) fn close(&mut self, stage: &'static str) -> Result<(), NativePipeError> {
        if self.value() == 0 {
            if let Some(owner) = &mut self.owner {
                owner.output = None;
            }
            return Ok(());
        }
        // SAFETY: this is the unique retained slot; failure leaves it unchanged.
        if let Err(cause) = unsafe { CloseHandle(self.raw()) } {
            let failure = error(stage, cause);
            self.retain(&failure);
            return Err(failure);
        }
        if let Some(owner) = &mut self.owner {
            owner.raw = 0;
            owner.output = None;
        }
        Ok(())
    }
    pub(crate) fn close_normal(&mut self, stage: &'static str) -> Result<(), NativePipeError> {
        self.check(stage)?;
        self.close(stage)?;
        self.check(stage)
    }
    pub(crate) fn retain(&mut self, failure: &NativePipeError) {
        if let Some(owner) = &mut self.owner {
            if let Some(first) = &mut owner.cause {
                first.retain_cleanup(failure.cause.clone());
            } else {
                owner.cause = Some(failure.cause.clone());
            }
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take()
            && owner.raw != 0
        {
            let retained = Box::leak(owner);
            tracing::error!(
                operation = "native_handle_uncertain_drop",
                retained_output = retained.output.is_some(),
                "Native handle custody retained without a checked close"
            );
        }
    }
}
pub(crate) fn adopt(
    slot: &mut Handle,
    raw: HANDLE,
    stage: &'static str,
) -> Result<(), NativePipeError> {
    slot.adopt_partial(raw);
    if raw.is_invalid() {
        return Err(error(stage, last_error()));
    }
    slot.check(stage)
}
pub(crate) fn last_error() -> windows::core::Error {
    // SAFETY: read this thread's immediate numeric Win32 error without borrowed storage.
    let code = unsafe { GetLastError() };
    windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(if code.0 == 0 {
        ERROR_GEN_FAILURE.0
    } else {
        code.0
    }))
}
pub(crate) fn code(error: &windows::core::Error, expected: WIN32_ERROR) -> bool {
    error.code() == windows::core::HRESULT::from_win32(expected.0)
}
pub(crate) fn error(
    stage: &'static str,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> NativePipeError {
    NativePipeError::native(NativeObservationError::new(stage, cause))
}

#[cfg(test)]
#[path = "handle_custody_tests.rs"]
mod tests;
