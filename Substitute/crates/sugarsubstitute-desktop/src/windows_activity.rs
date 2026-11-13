// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe only retained owned HWNDs and retire every subclass before native completion.
//!
//! All native calls, registrations and callbacks belong to the existing UI thread.
//! Provisional Registration ownership precedes installation; failed removal retains
//! the HWND and callback reference. No callback borrows content or creates work.

#[path = "windows_activity_state.rs"]
mod state;
use super::NativeAction;
use crate::{DesktopError, windows_subclass};
use state::{Signals, SurfaceFacts, Suspension};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use tao::{event_loop::EventLoopProxy, platform::windows::WindowExtWindows, window::Window};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::GetActiveWindow,
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            GetForegroundWindow, IsIconic, IsWindowVisible, WM_ACTIVATE, WM_ACTIVATEAPP,
            WM_ENTERMENULOOP, WM_ENTERSIZEMOVE, WM_EXITMENULOOP, WM_EXITSIZEMOVE, WM_NCDESTROY,
            WM_POWERBROADCAST, WM_SHOWWINDOW, WM_SIZE, WM_WINDOWPOSCHANGED,
        },
    },
};

struct Mailbox {
    signals: RefCell<Signals>,
    failure: Cell<Option<DesktopError>>,
    callbacks: Cell<u32>,
    wake_pending: Cell<bool>,
    admitting: Cell<bool>,
}
impl Mailbox {
    fn new() -> Self {
        Self {
            signals: RefCell::new(Signals::new()),
            failure: Cell::new(None),
            callbacks: Cell::new(0),
            wake_pending: Cell::new(false),
            admitting: Cell::new(true),
        }
    }
    fn retain(&self, cause: DesktopError) {
        let mut first = self.failure.take();
        DesktopError::retain_first(&mut first, cause);
        self.failure.set(first);
    }
    fn change(&self, effect: impl FnOnce(&mut Signals)) {
        if !self.admitting.get() {
            return;
        }
        match self.signals.try_borrow_mut() {
            Ok(mut signals) => {
                effect(&mut signals);
                if let Some(cause) = signals.failure.take() {
                    self.retain(cause);
                }
            }
            Err(cause) => self.retain(DesktopError::new("native_activity_borrow", cause)),
        }
    }
    /// Claim the sole queued wake before sending, including synchronous reentrancy.
    fn wake(&self, send: impl FnOnce() -> Result<(), DesktopError>) {
        if !self.admitting.get() || self.wake_pending.get() {
            return;
        }
        let failure = self.failure.take();
        let dirty = failure.is_some()
            || self.signals.try_borrow().is_ok_and(|signals| signals.has_pending());
        self.failure.set(failure);
        if dirty {
            self.wake_pending.set(true);
            if let Err(cause) = send() {
                self.retain(cause);
                self.admitting.set(false);
            }
        }
    }
    fn consumed_wake(&self) {
        self.wake_pending.set(false);
    }
    fn close(&self) {
        self.admitting.set(false);
        match self.signals.try_borrow_mut() {
            Ok(mut signals) => signals.close(),
            Err(cause) => self.retain(DesktopError::new("native_activity_close", cause)),
        }
    }
}
struct Callback {
    mailbox: Rc<Mailbox>,
    proxy: EventLoopProxy<NativeAction>,
    slot: usize,
}
struct Observer {
    window: Option<Rc<Window>>,
    registration: windows_subclass::Registration<Callback>,
    detached: bool,
}
impl Observer {
    fn new(window: Rc<Window>, callback: Rc<Callback>) -> Result<Self, DesktopError> {
        let mut owner = Self {
            window: Some(window),
            registration: windows_subclass::Registration::new(&callback),
            detached: false,
        };
        let hwnd = owner.hwnd()?;
        // SAFETY: The exact owned window and Callback type are retained on this UI thread.
        // Provisional registration custody is established before SetWindowSubclass can reenter.
        let result =
            unsafe { windows_subclass::install(hwnd, Some(activity_proc), &owner.registration) };
        if result.is_err() && !owner.registration.installed() {
            owner.detached = true;
        }
        result?;
        Ok(owner)
    }
    fn hwnd(&self) -> Result<HWND, DesktopError> {
        self.window.as_ref().map(hwnd).ok_or_else(|| {
            DesktopError::message("native_activity_window", "owned window unavailable")
        })
    }
    fn shutdown(&mut self) -> Result<(), DesktopError> {
        if !self.detached {
            windows_subclass::remove(self.hwnd()?, Some(activity_proc), &self.registration)?;
            self.detached = true;
        }
        Ok(())
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        if !self.detached && self.registration.installed() {
            std::mem::forget(self.window.take());
            tracing::error!(
                operation = "native_activity_unresolved_drop",
                "Retaining unresolved activity window and callback custody"
            );
        }
    }
}

/// Two exact native surfaces share one bounded immutable activity stream.
pub(super) struct Observers {
    slots: [Option<Observer>; 2],
    mailbox: Rc<Mailbox>,
    proxy: EventLoopProxy<NativeAction>,
    closing: bool,
}
impl Observers {
    pub(super) fn new(proxy: EventLoopProxy<NativeAction>) -> Self {
        Self { slots: [None, None], mailbox: Rc::new(Mailbox::new()), proxy, closing: false }
    }
    /// Clear only the actually consumed queued Activity event, before reentrant UI work.
    pub(super) fn consumed_wake(&self) {
        self.mailbox.consumed_wake();
    }
    /// Bind only borrowed native window owners. Replacements cannot discard installed custody.
    pub(super) fn synchronize(
        &mut self,
        windows: [Option<&Rc<Window>>; 2],
    ) -> Result<(), DesktopError> {
        for (slot, window) in windows.into_iter().enumerate() {
            let window = if self.closing { None } else { window };
            let matching = self.slots[slot].as_ref().is_some_and(|owner| {
                !owner.registration.destroyed()
                    && owner
                        .window
                        .as_ref()
                        .zip(window)
                        .is_some_and(|(old, current)| Rc::ptr_eq(old, current))
            });
            if !matching {
                if let Some(owner) = &mut self.slots[slot] {
                    owner.shutdown()?;
                    if self.mailbox.callbacks.get() != 0 {
                        continue;
                    }
                    self.slots[slot] = None;
                    self.mailbox.change(|signals| signals.remove(slot));
                }
                if let Some(window) = window {
                    let callback = Rc::new(Callback {
                        mailbox: self.mailbox.clone(),
                        proxy: self.proxy.clone(),
                        slot,
                    });
                    self.slots[slot] = Some(Observer::new(window.clone(), callback)?);
                }
            }
            if let Some(owner) = &self.slots[slot] {
                let facts = sample(owner.hwnd()?);
                self.mailbox.change(|signals| signals.surface(slot, facts));
            }
        }
        self.take_failure()
    }
    pub(super) fn take(
        &mut self,
    ) -> Result<Option<sugarsubstitute_contracts::lifecycle::UiWatchdogActivity>, DesktopError>
    {
        self.take_failure()?;
        self.mailbox
            .signals
            .try_borrow_mut()
            .map(|mut signals| signals.take())
            .map_err(|cause| DesktopError::new("native_activity_drain", cause))
    }
    fn take_failure(&self) -> Result<(), DesktopError> {
        if let Some(cause) = self.mailbox.failure.take() {
            return Err(cause);
        }
        let mut signals = self
            .mailbox
            .signals
            .try_borrow_mut()
            .map_err(|cause| DesktopError::new("native_activity_failure", cause))?;
        if let Some(cause) = signals.failure.take() {
            return Err(cause);
        }
        Ok(())
    }
    pub(super) fn close(&mut self) {
        self.closing = true;
        self.mailbox.close();
    }
    pub(super) fn retired(&self) -> bool {
        self.closing && self.slots.iter().all(Option::is_none) && self.mailbox.callbacks.get() == 0
    }
    pub(super) fn live(&self) -> bool {
        !self.closing
            && self
                .slots
                .iter()
                .flatten()
                .any(|owner| !owner.detached && !owner.registration.destroyed())
    }
}
fn hwnd(window: &Rc<Window>) -> HWND {
    HWND(window.hwnd() as *mut std::ffi::c_void)
}
fn sample(hwnd: HWND) -> SurfaceFacts {
    // SAFETY: The caller retains this exact native window on its creating UI thread.
    unsafe {
        SurfaceFacts::observed(
            IsWindowVisible(hwnd).as_bool(),
            IsIconic(hwnd).as_bool(),
            GetActiveWindow() == hwnd && GetForegroundWindow() == hwnd,
        )
    }
}
fn suspension(message: u32, value: usize) -> Option<Suspension> {
    match message {
        WM_ENTERMENULOOP => Some(Suspension::Menu(true)),
        WM_EXITMENULOOP => Some(Suspension::Menu(false)),
        WM_ENTERSIZEMOVE => Some(Suspension::Move(true)),
        WM_EXITSIZEMOVE => Some(Suspension::Move(false)),
        // Windows PBT_APMSUSPEND=4, PBT_APMRESUMECRITICAL=6,
        // PBT_APMRESUMESUSPEND=7 and PBT_APMRESUMEAUTOMATIC=18 are acknowledged OS facts.
        WM_POWERBROADCAST if value == 4 => Some(Suspension::System(true)),
        WM_POWERBROADCAST if [6, 7, 18].contains(&value) => Some(Suspension::System(false)),
        _ => None,
    }
}
/// Keep callback depth live through the previous native procedure, even if it reenters teardown.
struct CallbackGuard(Rc<Mailbox>);
impl Drop for CallbackGuard {
    fn drop(&mut self) {
        self.0.callbacks.set(self.0.callbacks.get() - 1);
    }
}
unsafe extern "system" fn activity_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    let (mailbox, proxy, slot) = {
        // SAFETY: This exact Callback registration remains live before native forwarding.
        let callback = unsafe { windows_subclass::callback::<Callback>(data) };
        (callback.mailbox.clone(), callback.proxy.clone(), callback.slot)
    };
    let guard = mailbox.callbacks.get().checked_add(1).map(|depth| {
        mailbox.callbacks.set(depth);
        CallbackGuard(mailbox.clone())
    });
    if guard.is_none() {
        mailbox
            .retain(DesktopError::message("native_activity_callback", "callback depth exhausted"));
    }
    if let Some(reason) = suspension(message, wparam.0) {
        mailbox.change(|signals| signals.suspend(slot, reason));
    } else if [WM_ACTIVATE, WM_ACTIVATEAPP, WM_SHOWWINDOW, WM_SIZE, WM_WINDOWPOSCHANGED]
        .contains(&message)
    {
        mailbox.change(|signals| signals.surface(slot, sample(hwnd)));
    } else if message == WM_NCDESTROY {
        mailbox.change(|signals| signals.remove(slot));
    }
    mailbox.wake(|| {
        proxy
            .send_event(NativeAction::Activity)
            .map_err(|cause| DesktopError::new("wake_native_activity", cause))
    });
    let result = if message == WM_NCDESTROY {
        // SAFETY: Final native destruction chains first and returns exactly this registration reference.
        unsafe {
            windows_subclass::destroy_callback::<Callback>(
                hwnd,
                Some(activity_proc),
                wparam,
                lparam,
                data,
            )
        }
    } else {
        // SAFETY: Preserve the actual owned HWND's complete previous native procedure chain.
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    };
    drop(guard);
    result
}

#[cfg(test)]
#[path = "windows_activity_tests.rs"]
mod tests;
