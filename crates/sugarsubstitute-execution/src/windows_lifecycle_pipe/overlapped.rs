// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin one operation's event, OVERLAPPED and callback through actual completion/quiescence.
use super::*;
use crate::native::wait::Wait;
use std::error::Error;
pub(crate) enum AcceptedIssue {
    Pending,
    Synchronous,
    Connected,
}
pub(crate) struct Operation {
    event: Handle,
    overlapped: Box<OVERLAPPED>,
    wait: Option<Wait>,
    signal: Arc<Signal>,
    pending: bool,
    cancel_attempted: bool,
    completed: Option<u32>,
}
// SAFETY: the sole native worker exclusively accesses the pinned OVERLAPPED.
// Registered callbacks touch only the separate atomic Signal; moving this owner
// never moves its boxed storage, and retirement proves I/O and callback quiescence.
unsafe impl Send for Operation {}
impl Operation {
    pub(crate) fn new(signal: Arc<Signal>) -> Self {
        Self {
            event: Handle(0),
            overlapped: Box::new(OVERLAPPED::default()),
            wait: None,
            signal,
            pending: false,
            cancel_attempted: false,
            completed: None,
        }
    }
    fn begin_reuse(&mut self) -> Result<(), LifecyclePipeFailure> {
        if self.wait.is_some() || self.pending {
            return Err(failure("pipe_live_overlapped_reuse"));
        }
        // Old operation evidence must not survive a later preparation failure.
        self.completed = None;
        Ok(())
    }
    pub(crate) fn prepare(&mut self) -> Result<(), LifecyclePipeFailure> {
        self.begin_reuse()?;
        if self.event.0 == 0 {
            // SAFETY: initial-false manual-reset event is exclusively owned here.
            let event = unsafe { CreateEventW(None, true, false, None) }
                .map_err(|e| error("pipe_create_event", e))?;
            adopt(&mut self.event, event, "pipe_create_event")?;
        }
        // SAFETY: prior callback was quiesced before reusing the event/OVERLAPPED.
        unsafe { ResetEvent(self.event.raw()) }.map_err(|e| error("pipe_reset_event", e))?;
        *self.overlapped = OVERLAPPED { hEvent: self.event.raw(), ..Default::default() };
        self.cancel_attempted = false;
        self.signal.reset();
        self.wait = Some(Wait::for_operation(
            self.signal.clone(),
            "pipe_register_event",
            "pipe_unregister_event",
        ));
        self.wait
            .as_mut()
            .ok_or_else(|| failure("pipe_wait_owner"))?
            .register(self.event.raw())
            .map_err(LifecyclePipeFailure::native)
    }
    pub(crate) fn pointer(&mut self) -> *mut OVERLAPPED {
        self.overlapped.as_mut()
    }
    /// Adopt native acceptance before any fallible completion polling.
    pub(crate) fn accept(
        &mut self,
        result: windows::core::Result<()>,
        connect: bool,
    ) -> Result<AcceptedIssue, LifecyclePipeFailure> {
        match result {
            Ok(()) if connect => Ok(AcceptedIssue::Connected),
            Ok(()) => {
                self.pending = true;
                Ok(AcceptedIssue::Synchronous)
            }
            Err(e) if code(&e, ERROR_IO_PENDING) => {
                self.pending = true;
                Ok(AcceptedIssue::Pending)
            }
            Err(e) if connect && code(&e, ERROR_PIPE_CONNECTED) => Ok(AcceptedIssue::Connected),
            Err(e) => Err(error(if connect { "pipe_connect" } else { "pipe_issue" }, e)),
        }
    }
    /// Poll synchronous data counts only after acceptance has been preserved.
    pub(crate) fn initial_progress(
        &mut self,
        accepted: AcceptedIssue,
        pipe: HANDLE,
    ) -> Result<Progress, LifecyclePipeFailure> {
        match accepted {
            AcceptedIssue::Pending => Ok(Progress::Pending),
            AcceptedIssue::Connected => Ok(Progress::Complete(0)),
            AcceptedIssue::Synchronous => self.poll(pipe),
        }
    }
    pub(crate) fn poll(&mut self, pipe: HANDLE) -> Result<Progress, LifecyclePipeFailure> {
        if !self.pending {
            return Err(failure("pipe_poll_without_pending"));
        }
        let mut transferred = 0;
        // SAFETY: pipe and pinned OVERLAPPED/buffer remain live; this never waits.
        match unsafe {
            GetOverlappedResult(pipe, self.overlapped.as_ref(), &mut transferred, false)
        } {
            Ok(()) => {
                self.pending = false;
                self.completed = Some(transferred);
                Ok(Progress::Complete(transferred))
            }
            Err(e) if code(&e, ERROR_IO_INCOMPLETE) => Ok(Progress::Pending),
            Err(e) => {
                self.pending = false;
                Err(error("pipe_completion", e))
            }
        }
    }
    pub(crate) fn quiesce(&mut self) -> Result<(), LifecyclePipeFailure> {
        if self.pending {
            return Err(failure("pipe_quiesce_pending_io"));
        }
        if let Some(wait) = &mut self.wait {
            wait.retire().map_err(LifecyclePipeFailure::native)?;
        }
        self.wait = None;
        Ok(())
    }
    pub(crate) fn take_completed(&mut self) -> Option<u32> {
        if self.pending || self.wait.is_some() { None } else { self.completed.take() }
    }
    pub(crate) fn retire(&mut self, pipe: HANDLE) -> Result<bool, LifecyclePipeFailure> {
        if self.pending && !self.cancel_attempted {
            // SAFETY: cancellation names only this exact live operation; it proves no completion.
            if let Err(e) = unsafe { CancelIoEx(pipe, Some(self.overlapped.as_ref())) }
                && !code(&e, ERROR_NOT_FOUND)
            {
                return Err(error("pipe_cancel_io", e));
            }
            self.cancel_attempted = true;
        }
        if self.pending {
            match self.poll(pipe) {
                Ok(Progress::Pending) => return Ok(false),
                Ok(Progress::Complete(_)) => {}
                Err(e)
                    if e.cause
                        .source()
                        .and_then(|s| s.downcast_ref::<windows::core::Error>())
                        .is_some_and(|e| code(e, ERROR_OPERATION_ABORTED)) => {}
                Err(e) => return Err(e),
            }
        }
        self.quiesce()?;
        self.event.close("pipe_close_event")?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "overlapped_tests.rs"]
mod tests;
