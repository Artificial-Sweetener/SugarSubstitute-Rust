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
    accounting: Option<(
        Arc<sugarsubstitute_contracts::execution::NativeHandleClaim>,
        crate::native_resources::EffectContext,
    )>,
    handle: Handle,
    wait: Option<Wait>,
    signal: Arc<Signal>,
}
impl Event {
    /// Allocate inert owner storage before any external native effect.
    pub(crate) fn new(signal: Arc<Signal>) -> Self {
        Self { accounting: None, handle: Handle::new(), wait: None, signal }
    }
    /// Bind original admitted native-output credits before creation or registration.
    pub(crate) fn attach(
        &mut self,
        handles: Arc<sugarsubstitute_contracts::execution::NativeHandleClaim>,
        original: crate::native_resources::EffectContext,
    ) {
        self.handle.attach(handles.clone(), original.clone());
        self.accounting = Some((handles, original));
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
        if let Some((handles, original)) = &self.accounting
            && let Some(wait) = &mut self.wait
        {
            wait.attach(handles.clone(), original.clone());
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
    /// Perform checked event close only after the caller proves actual native I/O settlement.
    /// The original Event/Wait outputs and causes stay owned if either boundary fails.
    pub(crate) fn close(&mut self) -> Result<(), NativePipeError> {
        self.quiesce()?;
        self.handle.close("pipe_close_event")
    }
}
