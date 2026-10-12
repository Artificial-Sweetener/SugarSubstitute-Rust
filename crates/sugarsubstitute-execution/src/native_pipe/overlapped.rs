// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin one operation's event, OVERLAPPED and callback through actual completion/quiescence.
use super::event::Event;
use super::{NativePipeError, Progress, handles::*};
use crate::native::Signal;
use std::error::Error;
use std::sync::Arc;
use windows::Win32::{Foundation::*, System::IO::*};
pub(crate) enum AcceptedIssue {
    Pending,
    Synchronous,
    Connected,
}
pub(crate) struct Operation {
    event: Event,
    overlapped: Box<OVERLAPPED>,
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
            event: Event::new(signal),
            overlapped: Box::new(OVERLAPPED::default()),
            pending: false,
            cancel_attempted: false,
            completed: None,
        }
    }
    pub(crate) fn attach(
        &mut self,
        handles: Arc<sugarsubstitute_contracts::execution::NativeHandleClaim>,
        original: crate::native_resources::EffectContext,
    ) {
        self.event.attach(handles, original);
    }
    fn begin_reuse(&mut self) -> Result<(), NativePipeError> {
        if self.event.registered() || self.pending {
            return Err(invariant("pipe_live_overlapped_reuse"));
        }
        // Old operation evidence must not survive a later preparation failure.
        self.completed = None;
        Ok(())
    }
    pub(crate) fn prepare(&mut self) -> Result<(), NativePipeError> {
        self.begin_reuse()?;
        self.event.prepare()?;
        *self.overlapped = OVERLAPPED { hEvent: self.event.raw(), ..Default::default() };
        self.cancel_attempted = false;
        Ok(())
    }
    pub(crate) fn pointer(&mut self) -> *mut OVERLAPPED {
        self.overlapped.as_mut()
    }
    /// Adopt native acceptance before any fallible completion polling.
    pub(crate) fn accept(
        &mut self,
        result: windows::core::Result<()>,
        connect: bool,
    ) -> Result<AcceptedIssue, NativePipeError> {
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
    /// Issue a finite transaction using this exact prepared operation's custody.
    /// The caller owns both fixed buffers through completion/quiescence and holds
    /// its actual issue fence. Acceptance precedes every completion/status query.
    pub(crate) fn transact(
        &mut self,
        pipe: HANDLE,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<AcceptedIssue, NativePipeError> {
        let input_length =
            u32::try_from(input.len()).map_err(|_| invariant("pipe_transaction_input_bound"))?;
        let output_length =
            u32::try_from(output.len()).map_err(|_| invariant("pipe_transaction_output_bound"))?;
        // SAFETY: caller retains both bounded allocations and this pinned
        // OVERLAPPED until actual completion, cancellation and callback quiescence.
        let result = unsafe {
            windows::Win32::System::Pipes::TransactNamedPipe(
                pipe,
                Some(input.as_ptr().cast()),
                input_length,
                Some(output.as_mut_ptr().cast()),
                output_length,
                std::ptr::null_mut(),
                Some(self.pointer()),
            )
        };
        self.accept(result, false)
    }
    /// Poll synchronous data counts only after acceptance has been preserved.
    pub(crate) fn initial_progress(
        &mut self,
        accepted: AcceptedIssue,
        pipe: HANDLE,
    ) -> Result<Progress, NativePipeError> {
        self.check("pipe_issue_accepted")?;
        match accepted {
            AcceptedIssue::Pending => Ok(Progress::Pending),
            AcceptedIssue::Connected => Ok(Progress::Complete(0)),
            AcceptedIssue::Synchronous => self.poll(pipe),
        }
    }
    fn check(&self, stage: &'static str) -> Result<(), NativePipeError> {
        self.event.check(stage)
    }
    pub(crate) fn poll(&mut self, pipe: HANDLE) -> Result<Progress, NativePipeError> {
        self.check("pipe_completion")?;
        let result = self.poll_retirement(pipe);
        self.check("pipe_completion")?;
        result
    }
    fn poll_retirement(&mut self, pipe: HANDLE) -> Result<Progress, NativePipeError> {
        if !self.pending {
            return Err(invariant("pipe_poll_without_pending"));
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
    pub(crate) fn quiesce(&mut self) -> Result<(), NativePipeError> {
        if self.pending {
            return Err(invariant("pipe_quiesce_pending_io"));
        }
        self.event.quiesce()
    }
    pub(crate) fn take_completed(&mut self) -> Option<u32> {
        if self.pending || self.event.registered() { None } else { self.completed.take() }
    }
    pub(crate) fn retire(&mut self, pipe: HANDLE) -> Result<bool, NativePipeError> {
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
            match self.poll_retirement(pipe) {
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
        self.event.close()?;
        Ok(true)
    }
}

fn invariant(stage: &'static str) -> NativePipeError {
    error(stage, std::io::Error::other("Native pipe operation invariant rejected"))
}

#[cfg(test)]
#[path = "overlapped_tests.rs"]
mod tests;
