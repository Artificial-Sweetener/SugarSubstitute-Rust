// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep stable read storage and callback ownership through real cancellation completion.
use super::*;
use crate::native::wait::Wait;
pub(super) struct Operation {
    event: Handle,
    overlapped: Box<OVERLAPPED>,
    wait: Option<Wait>,
    signal: Arc<Signal>,
    pending: bool,
    cancel_attempted: bool,
    completed: Option<Progress>,
}
// SAFETY: only the single native worker accesses boxed OVERLAPPED and the owned
// event. Callbacks access a separate Signal. Checked retirement proves both I/O
// and callback quiescence before freeing the stable owner.
unsafe impl Send for Operation {}
impl Operation {
    pub(super) fn new(signal: Arc<Signal>) -> Self {
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
    fn prepare(&mut self) -> Result<(), NativeObservationError> {
        if self.pending || self.wait.is_some() {
            return Err(invariant("output_live_operation_reuse"));
        }
        self.completed = None;
        if self.event.0 == 0 {
            // SAFETY: create an initial-false exclusively owned manual-reset event.
            let event = unsafe { CreateEventW(None, true, false, None) }
                .map_err(|e| error("output_create_event", e))?;
            self.event.adopt(event, "output_create_event")?;
        }
        // SAFETY: previous I/O and callback are quiescent before this event is reused.
        unsafe { ResetEvent(self.event.raw()) }.map_err(|e| error("output_reset_event", e))?;
        *self.overlapped = OVERLAPPED { hEvent: self.event.raw(), ..Default::default() };
        self.cancel_attempted = false;
        self.signal.reset();
        self.wait = Some(Wait::for_operation(
            self.signal.clone(),
            "output_register_read",
            "output_unregister_read",
        ));
        self.wait.as_mut().ok_or_else(|| invariant("output_wait_owner"))?.register(self.event.raw())
    }
    pub(super) fn read(
        &mut self,
        pipe: HANDLE,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        self.prepare()?;
        // Durable pending/storage state precedes Issue; unwind cannot discard a
        // buffer still referenced by kernel I/O. Counts come only from GetResult.
        self.pending = true;
        // SAFETY: admitted <=32KiB buffer and boxed OVERLAPPED remain pinned until
        // actual GetOverlappedResult settlement plus checked callback retirement.
        let result = unsafe { ReadFile(pipe, Some(bytes), None, Some(self.overlapped.as_mut())) };
        match result {
            Ok(()) => self.poll(pipe),
            Err(e) if code(&e, ERROR_IO_PENDING) => Ok(Progress::IssuedPending),
            Err(e) if code(&e, ERROR_BROKEN_PIPE) => {
                self.pending = false;
                self.completed = Some(Progress::Eof);
                Ok(Progress::Eof)
            }
            Err(e) => {
                self.pending = false;
                Err(error("output_issue_read", e))
            }
        }
    }
    pub(super) fn connect_owned(&mut self, pipe: HANDLE) -> Result<(), NativeObservationError> {
        self.prepare()?;
        self.pending = true;
        // SAFETY: parent has already opened/verified the sole own client. Even
        // unexpected pending connection retains its exact boxed operation until
        // checked cancellation settlement; no stack OVERLAPPED can escape.
        match unsafe { ConnectNamedPipe(pipe, Some(self.overlapped.as_mut())) } {
            Ok(()) => self.pending = false,
            Err(e) if code(&e, ERROR_PIPE_CONNECTED) => self.pending = false,
            Err(e) if code(&e, ERROR_IO_PENDING) => {
                return Err(invariant("output_own_connect_pending"));
            }
            Err(e) => {
                self.pending = false;
                return Err(error("output_connect_own_client", e));
            }
        }
        self.quiesce()
    }
    pub(super) fn poll(&mut self, pipe: HANDLE) -> Result<Progress, NativeObservationError> {
        if !self.pending {
            return Err(invariant("output_poll_without_pending"));
        }
        let mut bytes = 0;
        // SAFETY: exact stable live operation; this call is explicitly nonblocking.
        match unsafe { GetOverlappedResult(pipe, self.overlapped.as_ref(), &mut bytes, false) } {
            Ok(()) => {
                self.pending = false;
                let progress = if bytes == 0 { Progress::Empty } else { Progress::Bytes(bytes) };
                self.completed = Some(progress);
                Ok(progress)
            }
            Err(e) if code(&e, ERROR_IO_INCOMPLETE) => Ok(Progress::Pending),
            Err(e) if code(&e, ERROR_BROKEN_PIPE) => {
                self.pending = false;
                self.completed = Some(Progress::Eof);
                Ok(Progress::Eof)
            }
            Err(e) => {
                self.pending = false;
                Err(error("output_read_completion", e))
            }
        }
    }
    pub(super) fn quiesce(&mut self) -> Result<(), NativeObservationError> {
        if self.pending {
            return Err(invariant("output_quiesce_pending"));
        }
        if let Some(wait) = &mut self.wait {
            wait.retire()?;
        }
        self.wait = None;
        Ok(())
    }
    pub(super) fn take_completed(&mut self) -> Option<Progress> {
        if self.pending || self.wait.is_some() { None } else { self.completed.take() }
    }
    pub(super) fn retire(&mut self, pipe: HANDLE) -> Result<bool, NativeObservationError> {
        if self.pending && !self.cancel_attempted {
            // SAFETY: cancel only the exact retained operation, never claim completion.
            if let Err(e) = unsafe { CancelIoEx(pipe, Some(self.overlapped.as_ref())) }
                && !code(&e, ERROR_NOT_FOUND)
            {
                return Err(error("output_cancel_read", e));
            }
            self.cancel_attempted = true;
        }
        if self.pending {
            match self.poll(pipe) {
                Ok(Progress::Pending) => return Ok(false),
                Ok(_) => {}
                Err(e) => {
                    use std::error::Error;
                    if !e
                        .source()
                        .and_then(|s| s.downcast_ref::<windows::core::Error>())
                        .is_some_and(|e| code(e, ERROR_OPERATION_ABORTED))
                    {
                        return Err(e);
                    }
                }
            }
        }
        self.quiesce()?;
        self.event.close("output_close_read_event")?;
        Ok(true)
    }
}
