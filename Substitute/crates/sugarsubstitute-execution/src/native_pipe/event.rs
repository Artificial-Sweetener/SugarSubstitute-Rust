// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one actual completion event and registered callback through checked retirement.
use super::{NativePipeError, handles::*};
use crate::native::{Signal, wait::Wait};
use std::sync::Arc;
use windows::Win32::{
    Foundation::HANDLE,
    System::Threading::{CreateEventW, ResetEvent},
};

/// Caller retains its actual native I/O until this event/wait may be retired.
/// Failed creation/registration/closure keeps original accounting and native outputs.
pub(crate) struct Event {
    accounting: Option<super::accounting::OutputAccounting>,
    handle: Handle,
    wait: Option<Wait>,
    signal: Arc<Signal>,
}
impl Event {
    /// Count actual Event plus separately allocated Handle/Wait/Callback cells.
    /// Inline Handle/Wait values already counted in Event are subtracted once.
    pub(crate) const fn storage_bytes() -> usize {
        size_of::<Self>() + Handle::storage_bytes() - size_of::<Handle>() + Wait::storage_bytes()
            - size_of::<Wait>()
    }
    /// Allocate inert owner storage before any external native effect.
    pub(crate) fn new(signal: Arc<Signal>) -> Self {
        Self { accounting: None, handle: Handle::new(), wait: None, signal }
    }
    /// Bind original admitted native-output credits before creation or registration.
    pub(crate) fn attach(&mut self, accounting: super::accounting::OutputAccounting) {
        self.handle.attach(accounting.clone());
        self.accounting = Some(accounting);
    }
    /// Create/reset/register outside locks after preceding I/O and callbacks quiesced.
    pub(crate) fn prepare(&mut self) -> Result<(), NativePipeError> {
        if self.wait.is_some() {
            return Err(error(
                "pipe_live_event_reuse",
                std::io::Error::other("Event callback remains held"),
            ));
        }
        if self.handle.value() == 0 {
            self.handle.reserve("pipe_create_event")?;
            // SAFETY: initial-false manual-reset event is exclusively owned here.
            let event = unsafe { CreateEventW(None, true, false, None) }
                .map_err(|e| error("pipe_create_event", e))?;
            adopt(&mut self.handle, event, "pipe_create_event")?;
        }
        self.handle.check("pipe_reset_event")?;
        // SAFETY: prior callback was quiesced before reusing the event/OVERLAPPED.
        unsafe { ResetEvent(self.handle.raw()) }.map_err(|e| error("pipe_reset_event", e))?;
        self.handle.check("pipe_reset_event")?;
        self.signal.reset();
        self.wait = Some(Wait::for_operation(
            self.signal.clone(),
            "pipe_register_event",
            "pipe_unregister_event",
        ));
        if let Some(accounting) = &self.accounting
            && let Some(wait) = &mut self.wait
        {
            wait.attach(accounting.clone());
        }
        self.wait
            .as_mut()
            .ok_or_else(|| {
                error("pipe_wait_owner", std::io::Error::other("Actual event wait owner missing"))
            })?
            .register(self.handle.raw())
            .map_err(NativePipeError::native)
    }
    /// Borrow the exact retained event in the narrow same-worker native adapter.
    pub(crate) fn raw(&self) -> HANDLE {
        self.handle.raw()
    }
    /// Observe callback custody without a provider/native call or fresh authority.
    pub(crate) fn registered(&self) -> bool {
        self.wait.is_some()
    }
    /// Observe only the actual owner's adopted checked closure facts.
    /// NoLoan/native I/O status alone cannot make this predicate true.
    pub(crate) fn settled(&self) -> bool {
        self.handle.value() == 0 && self.wait.is_none()
    }
    /// Match adopted closure to this exact registered original counter allocation.
    /// This passive comparison issues no new effect/proof and samples no provider.
    pub(crate) fn settled_for(
        &self,
        original: &crate::capture_bundle::CaptureBundleOrigin,
        identity: usize,
    ) -> bool {
        self.settled()
            && matches!(
                &self.accounting,
                Some(super::accounting::OutputAccounting::Capture(claim))
                    if identity != 0
                        && claim.identity() == identity
                        && claim.original().same_allocation(original)
            )
    }
    /// Check original effect permission outside locks without altering custody.
    pub(crate) fn check(&self, stage: &'static str) -> Result<(), NativePipeError> {
        self.handle.check(stage)
    }
    /// Check callback completion after actual I/O retirement; failure retains the Wait.
    pub(crate) fn quiesce(&mut self) -> Result<(), NativePipeError> {
        if let Some(wait) = &mut self.wait {
            wait.retire().map_err(NativePipeError::native)?;
        }
        self.wait = None;
        Ok(())
    }
    /// Quiesce the actual child callback under its outer-held original cleanup proof.
    pub(crate) fn quiesce_capture(
        &mut self,
        proof: &crate::capture_bundle::CaptureBundleRetirement,
    ) -> Result<(), NativePipeError> {
        if let Some(wait) = &mut self.wait {
            wait.retire_capture(proof).map_err(NativePipeError::native)?;
        }
        self.wait = None;
        Ok(())
    }
    /// Close only after C++ I/O retirement and actual callback quiescence.
    /// Every fresh effect checks the same original proof; late returns adopt first.
    pub(crate) fn close_capture(
        &mut self,
        proof: &crate::capture_bundle::CaptureBundleRetirement,
    ) -> Result<(), NativePipeError> {
        self.quiesce_capture(proof)?;
        if self.handle.value() == 0 {
            return Ok(());
        }
        self.accounting
            .as_ref()
            .ok_or_else(|| {
                error(
                    "capture_event_accounting",
                    std::io::Error::other("Actual child Event accounting missing"),
                )
            })?
            .check_retirement(proof)
            .map_err(|cause| error("capture_close_event", cause))?;
        self.handle.close("capture_close_event")
    }
    /// Perform checked event close only after the caller proves actual native I/O settlement.
    /// The original Event/Wait outputs and causes stay owned if either boundary fails.
    pub(crate) fn close(&mut self) -> Result<(), NativePipeError> {
        if self.accounting.as_ref().is_some_and(|accounting| accounting.capture()) {
            return Err(error(
                "capture_event_cleanup_proof",
                std::io::Error::other("Child event requires original sealed cleanup proof"),
            ));
        }
        self.quiesce()?;
        self.handle.close("pipe_close_event")
    }
}
