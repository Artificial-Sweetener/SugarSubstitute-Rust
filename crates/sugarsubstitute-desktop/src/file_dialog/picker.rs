// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run queued shell requests exclusively from Tao's pre-dispatch hook on the owning UI STA.
use super::*;
use crate::DesktopError;
use std::{cell::Cell, rc::Rc};
use tao::{event_loop::EventLoopBuilder, platform::windows::EventLoopBuilderExtWindows};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_NULL},
};

/// Owned shell result; cancellation retains no filesystem authority.
type SelectionResult = Result<Option<PathBuf>, Box<dyn Error>>;
/// Same-thread notification that posts an event without borrowing application state.
type Signal = Box<dyn Fn() -> Result<(), DesktopError>>;
/// A retired binding can never be attached to another native peer.
#[derive(Clone, Copy)]
enum Peer {
    Unbound,
    Bound(HWND),
    Retired,
}
/// Fixed request/result storage retained by the host and its installed message hook.
struct State {
    peer: Cell<Peer>,
    pending: Cell<Option<SelectionKind>>,
    busy: Cell<bool>,
    active: ActiveDialog,
    result: RefCell<Option<SelectionResult>>,
    failure: RefCell<Option<DesktopError>>,
    select: Selection,
    signal: RefCell<Option<Signal>>,
}
impl State {
    /// Preserve the first notification failure independently of the shell result.
    fn signal(&self) {
        if let Some(signal) = self.signal.borrow().as_ref()
            && let Err(cause) = signal()
        {
            let mut failure = self.failure.borrow_mut();
            if failure.is_none() {
                *failure = Some(cause);
            }
        }
    }
    /// Only the installed pre-dispatch hook can enter the external modal boundary.
    fn message_turn(&self) {
        let Peer::Bound(window) = self.peer.get() else {
            return;
        };
        let Some(kind) = self.pending.take() else {
            return;
        };
        let selected = (self.select)(window, kind, &self.active);
        *self.result.borrow_mut() = Some(selected);
        self.signal();
    }
}

/// Own one queued/active/result slot on the creating UI STA. The host retains the original
/// HWND until the slot settles, retires this owner, then destroys the peer and event loop.
#[derive(Clone)]
pub(crate) struct Picker(Rc<State>);
impl Picker {
    /// Install the sole dispatch path before creating the event loop. Tao calls this supported
    /// hook outside its application-handler borrow; the shell can safely pump nested events.
    pub(crate) fn new<T: 'static>(builder: &mut EventLoopBuilder<T>) -> Self {
        Self::install(builder, Box::new(choose))
    }
    /// Install a single hook; the external selector is never exposed to application callbacks.
    fn install<T: 'static>(builder: &mut EventLoopBuilder<T>, select: Selection) -> Self {
        let state = Rc::new(State {
            peer: Cell::new(Peer::Unbound),
            pending: Cell::new(None),
            busy: Cell::new(false),
            active: ActiveDialog::default(),
            result: RefCell::new(None),
            failure: RefCell::new(None),
            select,
            signal: RefCell::new(None),
        });
        let native = state.clone();
        builder.with_msg_hook(move |_| {
            native.message_turn();
            false
        });
        Self(state)
    }
    /// Replace the shell alone; Tao's actual pre-dispatch and nested native pumping stay intact.
    #[cfg(test)]
    pub(crate) fn with_selection<T: 'static>(
        builder: &mut EventLoopBuilder<T>,
        select: Selection,
    ) -> Self {
        Self::install(builder, select)
    }
    /// Bind one original HWND and a bounded event-loop wake without retaining the window itself.
    /// # Errors
    /// Rejects duplicate or invalid peer attachment.
    pub(crate) fn bind(
        &self,
        window: HWND,
        signal: impl Fn() -> Result<(), DesktopError> + 'static,
    ) -> Result<(), DesktopError> {
        if window.is_invalid() || !matches!(self.0.peer.get(), Peer::Unbound) {
            return Err(DesktopError::message(
                "bind_native_picker",
                "Native picker requires one original window",
            ));
        }
        self.0.peer.set(Peer::Bound(window));
        *self.0.signal.borrow_mut() = Some(Box::new(signal));
        Ok(())
    }
    /// Whether one request owns the slot, including a modal call or an unconsumed result.
    pub(crate) fn busy(&self) -> bool {
        self.0.busy.get()
    }
    /// Queue a selection. This method never opens a dialog or enters a native modal loop.
    /// # Errors
    /// Rejects duplicate/retired requests and preserves the actual posting failure.
    pub(crate) fn request(&self, kind: SelectionKind) -> Result<(), DesktopError> {
        let Peer::Bound(window) = self.0.peer.get() else {
            return Err(DesktopError::message(
                "request_native_picker",
                "Native picker is not attached",
            ));
        };
        if self.busy() {
            return Err(DesktopError::message("request_native_picker", "Native picker is busy"));
        }
        self.0.busy.set(true);
        self.0.pending.set(Some(kind));
        // SAFETY: The host retains its same-thread HWND; WM_NULL carries no borrowed data.
        if let Err(cause) = unsafe { PostMessageW(Some(window), WM_NULL, WPARAM(0), LPARAM(0)) } {
            self.0.pending.take();
            self.0.busy.set(false);
            return Err(DesktopError::new("request_native_picker", cause));
        }
        Ok(())
    }
    /// Consume the sole owned result. A result is published only after the modal call returns.
    pub(crate) fn take_result(&self) -> Option<SelectionResult> {
        let result = self.0.result.borrow_mut().take();
        if result.is_some() {
            self.0.busy.set(false);
        }
        result
    }
    /// Cancel queued/active selection on its UI STA. Busy remains true until the result returns.
    /// # Errors
    /// Preserves the shell's actual close failure.
    pub(crate) fn cancel(&self) -> Result<(), DesktopError> {
        if self.0.pending.take().is_some() {
            *self.0.result.borrow_mut() = Some(Ok(None));
            self.0.signal();
        }
        self.0.active.cancel()
    }
    /// Observe the first wake failure without consuming a selection result.
    pub(crate) fn check(&self) -> Result<(), DesktopError> {
        self.0.failure.borrow_mut().take().map_or(Ok(()), Err)
    }
    /// Seal dispatch and release the wake before destroying the original peer/event loop.
    /// # Errors
    /// Rejects retirement while native selection still owns a request.
    pub(crate) fn retire(&self) -> Result<(), DesktopError> {
        if self.busy() {
            return Err(DesktopError::message(
                "retire_native_picker",
                "Native selection has not returned",
            ));
        }
        self.0.peer.set(Peer::Retired);
        self.0.signal.borrow_mut().take();
        self.check()
    }
}

#[cfg(test)]
mod tests;
