// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Catch genuine current-context provider unwind before the synchronous C boundary returns.
use std::{
    any::Any,
    error::Error,
    fmt,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
};
use sugarsubstitute_contracts::execution::{ExecutionId, JobContext, Moment, TimeError};
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticStorageError, DiagnosticStorageFailureCode, ProtectedCaptureReadGuard,
};

const CURRENT: u32 = 0;
const CANCELLED: u32 = 1;
const TIMED_OUT: u32 = 2;
const FAILED: u32 = 3;
const ERROR_CANCELLED: u32 = 995;
const ERROR_TIMEOUT: u32 = 1460;
const ERROR_BRIDGE: u32 = 31;

/// Match the frozen x64 ABI03 source; no retained native token owns these pointers.
#[repr(C)]
pub(super) struct ControlSource<'call> {
    borrowed_state: usize,
    check: unsafe extern "C" fn(usize, *mut u32) -> u32,
    _call: PhantomData<&'call mut ()>,
}
const _: () = assert!(std::mem::size_of::<ControlSource<'_>>() == 16);
const _: () = assert!(std::mem::align_of::<ControlSource<'_>>() == 8);

/// Keep the original monotonic observation across calls without renewing a budget.
#[derive(Default)]
pub(super) struct ControlHistory {
    owner: Option<ExecutionId>,
    last: Option<Moment>,
}
/// Retain the actual typed clock failure or original opaque provider panic.
pub(super) enum ControlCause {
    Cancelled,
    TimedOut,
    MissingDeadline,
    ForeignClock,
    Clock(TimeError),
    Source(DiagnosticStorageError),
    Panic(Box<dyn Any + Send>),
}
impl fmt::Debug for ControlCause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Cancelled => "FinishedControlCancelled",
            Self::TimedOut => "FinishedControlTimedOut",
            Self::MissingDeadline => "FinishedControlMissingDeadline",
            Self::ForeignClock => "FinishedControlForeignClock",
            Self::Clock(_) => "FinishedControlClock",
            Self::Source(_) => "FinishedControlSource",
            Self::Panic(_) => "FinishedControlPanic",
        })
    }
}
impl fmt::Display for ControlCause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, out)
    }
}
impl Error for ControlCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(source) => Some(source),
            Self::Source(source) => Some(source),
            _ => None,
        }
    }
}
impl ControlCause {
    pub(super) fn panic_payload(&self) -> Option<&(dyn Any + Send)> {
        match self {
            Self::Panic(payload) => Some(payload.as_ref()),
            _ => None,
        }
    }
    pub(super) fn status(&self) -> (u32, u32) {
        match self {
            Self::Cancelled => (CANCELLED, ERROR_CANCELLED),
            Self::TimedOut => (TIMED_OUT, ERROR_TIMEOUT),
            Self::Source(source) if source.code() == DiagnosticStorageFailureCode::Cancelled => {
                (CANCELLED, ERROR_CANCELLED)
            }
            Self::Source(source) if source.code() == DiagnosticStorageFailureCode::Deadline => {
                (TIMED_OUT, ERROR_TIMEOUT)
            }
            _ => (FAILED, ERROR_BRIDGE),
        }
    }
}

/// Borrow exactly one genuine current context and its same-clock observation history.
/// The caller transfers the retained cause to its native/source custody after return.
pub(super) struct ControlInvocation<'a> {
    context: &'a JobContext,
    generation: Option<&'a ProtectedCaptureReadGuard<'a>>,
    history: &'a mut ControlHistory,
    failure: Option<ControlCause>,
    fenced: Option<&'a mut dyn FnMut() -> Result<(), DiagnosticStorageError>>,
}
impl<'a> ControlInvocation<'a> {
    pub(super) fn new(context: &'a JobContext, history: &'a mut ControlHistory) -> Self {
        Self { context, generation: None, history, failure: None, fenced: None }
    }
    /// Observe the actual source-open fence before each ordinary native effect.
    pub(super) fn for_generation(
        generation: &'a ProtectedCaptureReadGuard<'a>,
        history: &'a mut ControlHistory,
    ) -> Self {
        Self {
            context: generation.context(),
            generation: Some(generation),
            history,
            failure: None,
            fenced: None,
        }
    }
    pub(super) fn for_fenced_generation(
        generation: &'a ProtectedCaptureReadGuard<'a>,
        history: &'a mut ControlHistory,
        fenced: &'a mut dyn FnMut() -> Result<(), DiagnosticStorageError>,
    ) -> Self {
        Self {
            context: generation.context(),
            generation: Some(generation),
            history,
            failure: None,
            fenced: Some(fenced),
        }
    }
    /// Borrow only during one synchronous same-thread native call.
    /// The native owner never stores/copies this source into a retained token.
    pub(super) fn source(&mut self) -> ControlSource<'_> {
        ControlSource {
            borrowed_state: std::ptr::from_mut(self) as usize,
            check: current,
            _call: PhantomData,
        }
    }
    pub(super) fn take_failure(&mut self) -> Option<ControlCause> {
        self.failure.take()
    }
    fn sample(&mut self) -> Result<(), ControlCause> {
        if self.history.owner.is_some_and(|owner| owner != self.context.owner_id()) {
            return Err(ControlCause::ForeignClock);
        }
        self.history.owner = Some(self.context.owner_id());
        if let Some(check) = self.fenced.as_deref_mut() {
            check().map_err(ControlCause::Source)?;
        }
        if let Some(generation) = self.generation {
            generation.check_current().map_err(ControlCause::Source)?;
        }
        let now = self.context.now();
        if let Some(previous) = self.history.last {
            now.checked_duration_since(previous).map_err(ControlCause::Clock)?;
        }
        self.history.last = Some(now);
        let cancelled = self.context.cancellation_requested();
        let deadline = self.context.deadline().ok_or(ControlCause::MissingDeadline)?;
        if now >= deadline {
            return Err(ControlCause::TimedOut);
        }
        if cancelled {
            return Err(ControlCause::Cancelled);
        }
        Ok(())
    }
    fn observe(&mut self) -> (u32, u32) {
        if let Some(failure) = &self.failure {
            return failure.status();
        }
        let observed = catch_unwind(AssertUnwindSafe(|| self.sample()));
        let failure = match observed {
            Ok(Ok(())) => None,
            Ok(Err(failure)) => Some(failure),
            Err(payload) => Some(ControlCause::Panic(payload)),
        };
        self.failure = failure;
        self.failure.as_ref().map_or((CURRENT, 0), ControlCause::status)
    }
}

/// Observe only the live stack-owned source supplied by this invocation.
/// # Safety
/// Nonzero `state` must address the exclusive live `ControlInvocation` on this
/// calling thread. `cause` must address a writable u32. The native implementation
/// calls neither concurrently nor after return; its retained token contains no source.
unsafe extern "C" fn current(state: usize, cause: *mut u32) -> u32 {
    if cause.is_null() {
        return FAILED;
    }
    if state == 0 {
        // SAFETY: The caller's ABI contract supplies the nonnull writable output.
        unsafe { cause.write(ERROR_BRIDGE) };
        return FAILED;
    }
    // SAFETY: Only source() constructs this pointer, and the synchronous native
    // calling-thread lifetime/exclusivity contract is required above.
    let invocation = unsafe { &mut *(state as *mut ControlInvocation<'_>) };
    let (status, value) = invocation.observe();
    // SAFETY: The caller supplies a writable output for this synchronous observation.
    unsafe { cause.write(value) };
    status
}

#[cfg(test)]
#[path = "control/tests.rs"]
mod tests;
