// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the noninheritable manual-reset event and bounded pre-Tao Windows message wait.
use super::{
    EventBoundary, WaitOutcome,
    messages::{self, Drain, MessageBoundary},
};
use crate::DesktopError;
use std::{sync::Mutex, time::Instant};
use sugarsubstitute_contracts::execution::{ExecutionError, Operation};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{CreateEventW, GetCurrentThreadId, ResetEvent, SetEvent},
    UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
        PeekMessageW, QS_ALLINPUT, TranslateMessage, WM_QUIT,
    },
};

/// Numeric storage does not transfer handle privileges; every signal/reset/close is
/// serialized by this mutex. Main-thread waits borrow the handle only until return;
/// handoff/close run on that same thread after the wait has returned. Worker callers
/// can only signal through BootstrapWake and cannot access this native handle.
pub(super) struct NativeEvent {
    handle: Mutex<Option<usize>>,
    thread: u32,
}
impl NativeEvent {
    pub(super) fn new() -> Result<Self, DesktopError> {
        // SAFETY: No security attributes/name, so the created event is private and
        // noninheritable. This sole owner retains it through checked CloseHandle.
        let handle = unsafe { CreateEventW(None, true, false, None) }
            .map_err(|cause| DesktopError::new("bootstrap_event_create", cause))?;
        // SAFETY: The native adapter records its creating main thread, with no handle input.
        let thread = unsafe { GetCurrentThreadId() };
        Ok(Self { handle: Mutex::new(Some(handle.0 as usize)), thread })
    }
    fn affinity(&self) -> Result<(), DesktopError> {
        // SAFETY: No input or borrowed resource; compare only the current thread identity.
        if unsafe { GetCurrentThreadId() } != self.thread {
            return Err(DesktopError::message(
                "bootstrap_affinity",
                "Bootstrap pump changed thread",
            ));
        }
        Ok(())
    }
    fn event_operation(
        &self,
        operation: impl FnOnce(HANDLE) -> windows::core::Result<()>,
    ) -> Result<(), ExecutionError> {
        let handle = self.handle.lock().map_err(|_| wake_error("Bootstrap event lock poisoned"))?;
        let raw = handle.ok_or_else(|| wake_error("Bootstrap event already closed"))?;
        operation(HANDLE(raw as *mut _))
            .map_err(|cause| ExecutionError::new(Operation::NativeLifecycle, Box::new(cause)))
    }
}
fn wake_error(message: &'static str) -> ExecutionError {
    ExecutionError::new(Operation::NativeLifecycle, Box::new(std::io::Error::other(message)))
}
/// Round a future absolute deadline upward to milliseconds without rebasing it.
fn milliseconds(deadline: Option<Instant>, now: Instant) -> u32 {
    match deadline {
        None => u32::MAX,
        Some(deadline) => {
            let nanos = deadline.saturating_duration_since(now).as_nanos();
            u32::try_from(nanos.div_ceil(1_000_000)).unwrap_or(u32::MAX - 1).min(u32::MAX - 1)
        }
    }
}
impl EventBoundary for NativeEvent {
    fn signal(&self) -> Result<(), ExecutionError> {
        // SAFETY: The owned handle is locked against closure for this entire native call.
        self.event_operation(|handle| unsafe { SetEvent(handle) })
    }
    fn reset(&self) -> Result<(), ExecutionError> {
        // SAFETY: Same sole owned-event lock; no worker can close or replace this handle.
        self.event_operation(|handle| unsafe { ResetEvent(handle) })
    }
    fn wait(
        &self,
        deadline: Option<Instant>,
        use_event: bool,
    ) -> Result<WaitOutcome, DesktopError> {
        self.affinity()?;
        // Drain native input before observing the event: a continuously signalled
        // completion cannot starve quit/input. Each wait dispatches at most 32.
        match messages::drain(&mut NativeQueue) {
            Drain::Quit(status) => return Ok(WaitOutcome::Quit(status)),
            Drain::Dispatched => return Ok(WaitOutcome::Woken),
            Drain::Empty => {}
        }
        let raw = if use_event {
            Some(
                self.handle
                    .lock()
                    .map_err(|_| DesktopError::message("bootstrap_wait", "Event lock poisoned"))?
                    .ok_or_else(|| {
                        DesktopError::message("bootstrap_wait", "Event already closed")
                    })?,
            )
        } else {
            None
        };
        let handles = raw.map(|raw| [HANDLE(raw as *mut _)]);
        // SAFETY: The owner and this wait stay on the creating main thread; only
        // that thread can close after return. Workers can only signal under lock.
        // A failed event remains retained while closing uses messages/deadlines only.
        let result = unsafe {
            MsgWaitForMultipleObjectsEx(
                handles.as_ref().map(|h| h.as_slice()),
                milliseconds(deadline, Instant::now()),
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE,
            )
        };
        if result == WAIT_FAILED {
            return Err(DesktopError::new(
                "bootstrap_message_wait",
                windows::core::Error::from_thread(),
            ));
        }
        if result == WAIT_TIMEOUT {
            return Ok(WaitOutcome::Deadline);
        }
        if use_event && result == WAIT_OBJECT_0 {
            return Ok(WaitOutcome::Woken);
        }
        if result.0 != WAIT_OBJECT_0.0 + u32::from(use_event) {
            return Err(DesktopError::message(
                "bootstrap_message_wait",
                "Unexpected native wait result",
            ));
        }
        Ok(match messages::drain(&mut NativeQueue) {
            Drain::Quit(status) => WaitOutcome::Quit(status),
            Drain::Empty | Drain::Dispatched => WaitOutcome::Woken,
        })
    }
    fn close(&self) -> Result<(), DesktopError> {
        self.affinity()?;
        let mut handle = self
            .handle
            .lock()
            .map_err(|_| DesktopError::message("bootstrap_close", "Event lock poisoned"))?;
        if let Some(raw) = *handle {
            // SAFETY: All worker signals are fenced by the caller's wake lock;
            // this main thread has no active wait and this owner holds the sole handle.
            unsafe { CloseHandle(HANDLE(raw as *mut _)) }
                .map_err(|cause| DesktopError::new("bootstrap_event_close", cause))?;
            *handle = None;
        }
        Ok(())
    }
}
/// Borrow only this main thread's native queue; no scheduler or native window is created.
struct NativeQueue;
impl MessageBoundary for NativeQueue {
    type Message = MSG;
    fn take(&mut self) -> Option<MSG> {
        let mut message = MSG::default();
        // SAFETY: Writable stack MSG and removal from the calling main thread's queue.
        unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool().then_some(message)
    }
    fn quit(&self, message: &MSG) -> Option<i32> {
        (message.message == WM_QUIT).then_some(message.wParam.0 as i32)
    }
    fn dispatch(&mut self, message: MSG) {
        // SAFETY: The just-dequeued message stays on its original main thread.
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
impl Drop for NativeEvent {
    fn drop(&mut self) {
        let retained = match self.handle.get_mut() {
            Ok(h) => h.is_some(),
            Err(h) => h.into_inner().is_some(),
        };
        if retained {
            tracing::error!(
                operation = "bootstrap_event_unsettled_drop",
                "Retaining unresolved native completion event until process termination"
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn timeout_rounding_preserves_expired_equality_and_never_uses_infinite_for_finite_deadline() {
        let now = Instant::now();
        assert_eq!(milliseconds(None, now), u32::MAX);
        assert_eq!(milliseconds(Some(now), now), 0);
        assert_eq!(milliseconds(Some(now + Duration::from_nanos(1)), now), 1);
        assert_eq!(milliseconds(Some(now + Duration::from_millis(12)), now), 12);
        assert_eq!(
            milliseconds(Some(now + Duration::from_secs(u64::from(u32::MAX))), now),
            u32::MAX - 1
        );
    }
    #[test]
    fn ffi_pre_tao_event_signals_resets_waits_and_checked_closes_without_tao()
    -> Result<(), Box<dyn std::error::Error>> {
        let event = NativeEvent::new()?;
        event.signal()?;
        assert_eq!(event.wait(Some(Instant::now()), true)?, WaitOutcome::Woken);
        event.reset()?;
        assert_eq!(event.wait(Some(Instant::now()), true)?, WaitOutcome::Deadline);
        event.close()?;
        event.close()?;
        assert!(event.signal().is_err());
        Ok(())
    }
}
