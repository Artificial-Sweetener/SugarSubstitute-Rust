// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Keep registered callback storage alive until synchronous unregister proves quiescence.

use crate::native::{NativeObservationError, Signal};
use ::windows::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    System::Threading::{
        INFINITE, RegisterWaitForSingleObject, UnregisterWaitEx, WT_EXECUTEINWAITTHREAD,
        WT_EXECUTEONLYONCE,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct Callback {
    signal: Arc<Signal>,
    timed_out: AtomicBool,
}
struct Owner {
    handle: Option<usize>,
    callback: Box<Callback>,
    uncertain_registration: bool,
    accounting: Option<crate::native_pipe::accounting::OutputAccounting>,
    output: Option<crate::native_pipe::accounting::OutputCredit>,
    cause: Option<NativeObservationError>,
    register_stage: &'static str,
    retire_stage: &'static str,
}
pub(crate) struct Wait {
    owner: Option<Box<Owner>>,
}
impl Wait {
    /// Bound the actual wait owner and stable callback cells before allocating.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<Self>() + size_of::<Owner>() + size_of::<Callback>()
    }
    pub(crate) fn new(signal: Arc<Signal>) -> Self {
        Self::for_operation(signal, "register_process_wait", "unregister_process_wait")
    }
    pub(crate) fn for_operation(
        signal: Arc<Signal>,
        register_stage: &'static str,
        retire_stage: &'static str,
    ) -> Self {
        Self {
            owner: Some(Box::new(Owner {
                handle: None,
                callback: Box::new(Callback { signal, timed_out: AtomicBool::new(false) }),
                uncertain_registration: false,
                accounting: None,
                output: None,
                cause: None,
                register_stage,
                retire_stage,
            })),
        }
    }
    pub(crate) fn attach(&mut self, accounting: crate::native_pipe::accounting::OutputAccounting) {
        if let Some(owner) = &mut self.owner {
            owner.accounting = Some(accounting);
        }
    }
    pub(crate) fn retained(&self) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|owner| owner.handle.is_some() || owner.uncertain_registration)
    }
    pub(crate) fn register(&mut self, process: HANDLE) -> Result<(), NativeObservationError> {
        let owner = self.owner.as_mut().ok_or_else(|| {
            NativeObservationError::new("wait_owner", std::io::Error::other("Wait owner absent"))
        })?;
        if owner.handle.is_some() || owner.uncertain_registration || owner.output.is_some() {
            return Err(NativeObservationError::new(
                owner.register_stage,
                std::io::Error::other("Wait registration already retained"),
            ));
        }
        owner.check()?;
        if let Some(accounting) = &owner.accounting {
            owner.output = Some(accounting.reserve_output(owner.register_stage)?);
        }
        let mut handle = HANDLE::default();
        let context = owner.callback.as_ref() as *const Callback;
        // SAFETY: stable boxed callback precedes even reentrant delivery and stays retained
        // until synchronous unregister proves completion on a separate native owner thread.
        let result = unsafe {
            RegisterWaitForSingleObject(
                &mut handle,
                process,
                Some(callback),
                Some(context.cast()),
                INFINITE,
                WT_EXECUTEONLYONCE | WT_EXECUTEINWAITTHREAD,
            )
        };
        if !handle.is_invalid() {
            owner.handle = Some(handle.0 as usize);
        }
        if let Err(cause) = result {
            if owner.handle.is_none() {
                owner.output = None;
            }
            let failure = NativeObservationError::new(owner.register_stage, cause);
            owner.retain(&failure);
            return Err(failure);
        }
        if owner.handle.is_none() {
            owner.uncertain_registration = true;
            let failure = NativeObservationError::new(
                owner.register_stage,
                std::io::Error::other("Successful registration returned no wait identity"),
            );
            owner.retain(&failure);
            return Err(failure);
        }
        owner.check()
    }
    pub(crate) fn timed_out(&self) -> bool {
        self.owner.as_ref().is_some_and(|owner| owner.callback.timed_out.load(Ordering::Acquire))
    }
    pub(crate) fn retire(&mut self) -> Result<(), NativeObservationError> {
        if self
            .owner
            .as_ref()
            .and_then(|owner| owner.accounting.as_ref())
            .is_some_and(|accounting| accounting.capture())
        {
            return Err(NativeObservationError::new(
                "wait_capture_cleanup_proof",
                std::io::Error::other("Child wait requires original sealed cleanup proof"),
            ));
        }
        self.retire_native()
    }
    /// Check original child close-only authority before accepting one finite unregister.
    /// Actual late successful return clears this same retained handle/credit first.
    pub(crate) fn retire_capture(
        &mut self,
        proof: &crate::capture_bundle::CaptureBundleRetirement,
    ) -> Result<(), NativeObservationError> {
        let owner = self.owner.as_ref().ok_or_else(|| {
            NativeObservationError::new(
                "wait_capture_owner",
                std::io::Error::other("Original wait owner missing"),
            )
        })?;
        if owner.handle.is_none() && !owner.uncertain_registration {
            return Ok(());
        }
        owner
            .accounting
            .as_ref()
            .ok_or_else(|| {
                NativeObservationError::new(
                    "wait_capture_accounting",
                    std::io::Error::other("Original child accounting missing"),
                )
            })?
            .check_retirement(proof)
            .map_err(|cause| NativeObservationError::new(owner.retire_stage, cause))?;
        self.retire_native()
    }
    /// Invoke the existing sole unregister boundary after its caller accepted cleanup.
    fn retire_native(&mut self) -> Result<(), NativeObservationError> {
        self.retire_with(|handle| {
            // SAFETY: the exact registration is exclusively retained, outside its callback;
            // the sentinel waits for actual callback completion before storage can be reclaimed.
            unsafe { UnregisterWaitEx(handle, Some(INVALID_HANDLE_VALUE)) }
        })
    }
    fn retire_with(
        &mut self,
        unregister: impl FnOnce(HANDLE) -> windows::core::Result<()>,
    ) -> Result<(), NativeObservationError> {
        let owner = self.owner.as_mut().ok_or_else(|| {
            NativeObservationError::new("wait_owner", std::io::Error::other("Wait owner absent"))
        })?;
        if owner.uncertain_registration {
            return Err(NativeObservationError::new(
                owner.retire_stage,
                std::io::Error::other(
                    "Registration has no retirement identity; callback lifetime remains unresolved",
                ),
            ));
        }
        if let Some(handle) = owner.handle {
            if let Err(cause) = unregister(HANDLE(handle as *mut _)) {
                let failure = NativeObservationError::new(owner.retire_stage, cause);
                owner.retain(&failure);
                return Err(failure);
            }
            owner.handle = None;
            owner.output = None;
        }
        Ok(())
    }
}
impl Owner {
    fn check(&self) -> Result<(), NativeObservationError> {
        if let Some(context) = &self.accounting {
            context
                .check()
                .map_err(|cause| NativeObservationError::new(self.register_stage, cause))?;
        }
        Ok(())
    }
    fn retain(&mut self, cause: &NativeObservationError) {
        if let Some(first) = &mut self.cause {
            first.retain_cleanup(cause.clone());
        } else {
            self.cause = Some(cause.clone());
        }
    }
}
impl Drop for Wait {
    fn drop(&mut self) {
        let retained = self.retained();
        if let Some(owner) = self.owner.take()
            && retained
        {
            let retained = Box::leak(owner);
            tracing::error!(
                operation = "native_wait_uncertain_drop",
                stage = retained.retire_stage,
                "Native callback and output custody retained without proven quiescence"
            );
        }
    }
}
unsafe extern "system" fn callback(context: *mut std::ffi::c_void, timed_out: bool) {
    // SAFETY: register supplies this stable Callback pointer and synchronous
    // unregister prevents its reclamation until this invocation has returned.
    let callback = unsafe { &*context.cast::<Callback>() };
    callback.timed_out.store(timed_out, Ordering::Release);
    // Only owned atomics and private scheduler wake; no application callback,
    // Core mutex, native handle cleanup, allocation or user clock in this ABI body.
    callback.signal.complete();
}

#[cfg(test)]
#[path = "windows_wait_custody_tests.rs"]
mod tests;
