// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer Windows shutdown and logoff so the close save runs instead of an abrupt exit.
//!
//! Windows asks every top-level window with WM_QUERYENDSESSION before a shutdown, logoff or
//! Restart Manager close, then sends WM_ENDSESSION. Tao 0.37 answers the query with the default
//! TRUE and, in its hidden thread-target window, calls `std::process::exit(0)` on
//! WM_ENDSESSION(TRUE), which would end the process with no close, no save and no settlement.
//!
//! This owner subclasses that thread-target window and every visible top-level window this
//! process created on the UI thread, all answering from one state:
//!
//! - WM_QUERYENDSESSION records a [`SessionEndSignal::Ending`] and wakes the event loop, which
//!   starts the close save. While the close save is outstanding, the app's visible windows hold
//!   a block reason ("Saving your workspace") and every subclassed window answers FALSE; once
//!   it settled they answer TRUE. A critical shutdown cannot be held, so it is answered TRUE at
//!   once and the close save runs on its short critical budget.
//! - WM_ENDSESSION(TRUE) is answered 0 and never reaches Tao, so the process is never exited
//!   from inside the window procedure; it exits itself once its close finishes.
//! - WM_ENDSESSION(FALSE) means the session continues: block reasons are removed and a
//!   [`SessionEndSignal::Cancelled`] is reported.
//!
//! Nothing blocks or waits inside a window procedure. Every call runs on the UI thread that
//! owns these windows.
use crate::{DesktopError, windows_subclass};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::{
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, LRESULT, TRUE, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW, Shutdown::ShutdownBlockReasonCreate,
            Shutdown::ShutdownBlockReasonDestroy, Threading::GetCurrentThreadId,
        },
        UI::{
            Shell::DefSubclassProc,
            WindowsAndMessaging::{
                EnumThreadWindows, GCLP_HMODULE, GetClassLongPtrW, GetClassNameW, GetParent,
                IsWindowVisible, WM_ENDSESSION, WM_NCDESTROY, WM_QUERYENDSESSION,
            },
        },
    },
    core::{BOOL, PCWSTR},
};

/// The class Tao 0.37 registers for the hidden window whose WM_ENDSESSION exits the process.
pub(crate) const TAO_THREAD_TARGET_CLASS: &str = "Tao Thread Event Target";
/// WM_QUERYENDSESSION/WM_ENDSESSION `lParam` flag: the session ends regardless of answers.
const ENDSESSION_CRITICAL: isize = 0x4000_0000;

/// What a session end asked of the application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionEndSignal {
    /// The session is ending; close with the close save. `critical` sessions cannot be held.
    Ending {
        /// Whether Windows ends the session without waiting for any answer.
        critical: bool,
    },
    /// Windows or another application cancelled the session end; the session continues.
    Cancelled,
}

/// The one state every subclassed window answers from.
struct State {
    /// Whether the close save has not settled, so the session end must be held.
    outstanding: Cell<bool>,
    /// The NUL-terminated localized block reason.
    reason: Vec<u16>,
    /// The latest signal the event loop has not taken yet.
    signal: Cell<Option<SessionEndSignal>>,
    /// Windows currently holding a block reason.
    blocking: RefCell<Vec<HWND>>,
    /// Wakes the event loop to take the signal; never waits.
    wake: Box<dyn Fn()>,
}

impl State {
    fn report(&self, signal: SessionEndSignal) {
        self.signal.set(Some(signal));
        (self.wake)();
    }

    /// Answer a session-end query for `hwnd`, holding a block reason on a visible window.
    fn query(&self, hwnd: HWND, critical: bool, holds_reason: bool) -> LRESULT {
        self.report(SessionEndSignal::Ending { critical });
        if critical || !self.outstanding.get() {
            self.release();
            return LRESULT(1);
        }
        if holds_reason && !self.blocking.borrow().contains(&hwnd) {
            // SAFETY: `hwnd` is a live top-level window owned by this UI thread and `reason`
            // is a NUL-terminated wide string this state keeps alive.
            match unsafe { ShutdownBlockReasonCreate(hwnd, PCWSTR(self.reason.as_ptr())) } {
                Ok(()) => self.blocking.borrow_mut().push(hwnd),
                Err(cause) => {
                    tracing::warn!(operation = "session_end_block_reason", cause = %cause,
                    "The shutdown block reason could not be shown")
                }
            }
        }
        LRESULT(0)
    }

    /// Remove every block reason this state created.
    fn release(&self) {
        for hwnd in self.blocking.borrow_mut().drain(..) {
            // SAFETY: Each window was live when its reason was created on this thread; a
            // destroyed window reports an error, which only means Windows removed it already.
            if let Err(cause) = unsafe { ShutdownBlockReasonDestroy(hwnd) } {
                tracing::debug!(operation = "session_end_block_reason", cause = %cause,
                    "A shutdown block reason was already gone");
            }
        }
    }

    fn forget(&self, hwnd: HWND) {
        self.blocking.borrow_mut().retain(|held| *held != hwnd);
    }
}

/// One subclassed window and its retained registration.
struct Watched {
    hwnd: HWND,
    registration: windows_subclass::Registration<State>,
}

/// Session-end handling for the UI thread's windows.
pub(crate) struct SessionEndWatch {
    state: Rc<State>,
    target: Watched,
    windows: Vec<Watched>,
    module: HMODULE,
}

impl SessionEndWatch {
    /// Subclass Tao's thread-target window on this thread, showing `reason` while holding a
    /// session end, and call `wake` whenever a signal waits.
    /// # Errors
    /// Fails when Tao's thread-target window is absent, which means Tao changed underneath
    /// this owner, or its subclass cannot be installed.
    pub(crate) fn install(reason: &str, wake: impl Fn() + 'static) -> Result<Self, DesktopError> {
        let state = Rc::new(State {
            outstanding: Cell::new(true),
            reason: reason.encode_utf16().chain(std::iter::once(0)).collect(),
            signal: Cell::new(None),
            blocking: RefCell::new(Vec::new()),
            wake: Box::new(wake),
        });
        let target = thread_windows()
            .into_iter()
            .find(|hwnd| class_name(*hwnd).as_deref() == Some(TAO_THREAD_TARGET_CLASS))
            .ok_or_else(|| {
                DesktopError::message(
                    "install_session_end",
                    "Tao's thread-target window is absent on the UI thread",
                )
            })?;
        let target = watch(target, &state, Some(target_proc))?;
        // SAFETY: A null module name returns this executable's own module handle.
        let module = unsafe { GetModuleHandleW(PCWSTR::null()) }
            .map_err(|cause| DesktopError::new("install_session_end", cause))?;
        let mut owner = Self { state, target, windows: Vec::new(), module };
        owner.track()?;
        Ok(owner)
    }

    /// Subclass every visible top-level window this executable created on this thread and
    /// that is not yet watched, so each one answers a session end.
    /// # Errors
    /// Preserves a subclass installation failure.
    pub(crate) fn track(&mut self) -> Result<(), DesktopError> {
        self.windows.retain(|watched| !watched.registration.destroyed());
        for hwnd in thread_windows() {
            if hwnd == self.target.hwnd || self.windows.iter().any(|watched| watched.hwnd == hwnd) {
                continue;
            }
            // SAFETY: Read-only queries of a live window on this thread.
            let ours = unsafe {
                IsWindowVisible(hwnd).as_bool()
                    && GetParent(hwnd).is_err()
                    && GetClassLongPtrW(hwnd, GCLP_HMODULE) == self.module.0 as usize
            };
            if ours {
                self.windows.push(watch(hwnd, &self.state, Some(window_proc))?);
            }
        }
        Ok(())
    }

    /// Watch one specific application window, as `track` does for visible ones.
    /// # Errors
    /// Preserves a subclass installation failure.
    #[cfg(test)]
    fn attach(&mut self, hwnd: HWND) -> Result<(), DesktopError> {
        self.windows.push(watch(hwnd, &self.state, Some(window_proc))?);
        Ok(())
    }

    /// Record whether the close save is still outstanding, which decides every answer.
    pub(crate) fn set_outstanding(&self, outstanding: bool) {
        self.state.outstanding.set(outstanding);
        if !outstanding {
            self.state.release();
        }
    }

    /// Take the latest session-end signal, if one waits.
    pub(crate) fn take_signal(&self) -> Option<SessionEndSignal> {
        self.state.signal.take()
    }

    /// Remove every block reason and subclass this owner still holds.
    /// # Errors
    /// Preserves the first subclass removal failure; Win32 then keeps the reference until
    /// the window's final destruction.
    pub(crate) fn retire(self) -> Result<(), DesktopError> {
        self.state.release();
        let mut failure = None;
        for watched in self.windows.iter().chain(std::iter::once(&self.target)) {
            if watched.registration.destroyed() || !watched.registration.installed() {
                continue;
            }
            let procedure: windows::Win32::UI::Shell::SUBCLASSPROC =
                if watched.hwnd == self.target.hwnd {
                    Some(target_proc)
                } else {
                    Some(window_proc)
                };
            if let Err(cause) =
                windows_subclass::remove(watched.hwnd, procedure, &watched.registration)
            {
                DesktopError::retain_first(&mut failure, cause);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

fn watch(
    hwnd: HWND,
    state: &Rc<State>,
    procedure: windows::Win32::UI::Shell::SUBCLASSPROC,
) -> Result<Watched, DesktopError> {
    let registration = windows_subclass::Registration::new(state);
    // SAFETY: `hwnd` is a live window owned by this UI thread; the procedure treats its data
    // as the State this registration retains and releases it on final destruction.
    unsafe { windows_subclass::install(hwnd, procedure, &registration)? };
    Ok(Watched { hwnd, registration })
}

/// Every top-level window of the calling thread.
fn thread_windows() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, found: LPARAM) -> BOOL {
        // SAFETY: `found` is the Vec this call lends for the synchronous enumeration only.
        unsafe { (*(found.0 as *mut Vec<HWND>)).push(hwnd) };
        TRUE
    }
    let mut found = Vec::new();
    // SAFETY: The enumeration is synchronous and `found` outlives it. A thread without
    // windows reports failure, which is the empty result.
    let _ = unsafe {
        EnumThreadWindows(GetCurrentThreadId(), Some(collect), LPARAM(&raw mut found as isize))
    };
    found
}

/// The class name of `hwnd`, or `None` when it cannot be read.
fn class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0_u16; 256];
    // SAFETY: The buffer outlives the call and its length bounds the copy.
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    let length = usize::try_from(length).ok().filter(|length| *length > 0)?;
    String::from_utf16(&buffer[..length]).ok()
}

/// Shared answer for every watched window. `holds_reason` windows show the block reason.
///
/// # Safety
/// Called only by Win32 for an installed subclass whose data is a `State` registration.
unsafe fn answer(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    data: usize,
    holds_reason: bool,
    procedure: windows::Win32::UI::Shell::SUBCLASSPROC,
) -> LRESULT {
    match message {
        WM_QUERYENDSESSION => {
            // SAFETY: The registration keeps this State alive while the subclass is installed.
            let state = unsafe { windows_subclass::callback::<State>(data) };
            state.query(hwnd, lparam.0 & ENDSESSION_CRITICAL != 0, holds_reason)
        }
        WM_ENDSESSION => {
            // SAFETY: As above.
            let state = unsafe { windows_subclass::callback::<State>(data) };
            if wparam.0 == 0 {
                state.release();
                state.report(SessionEndSignal::Cancelled);
            }
            // Never chained: Tao would exit the process from inside this procedure.
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // SAFETY: As above; final destruction returns this registration's reference.
            unsafe {
                windows_subclass::callback::<State>(data).forget(hwnd);
                windows_subclass::destroy_callback::<State>(hwnd, procedure, wparam, lparam, data)
            }
        }
        // SAFETY: Every other message keeps the window's own procedure chain.
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}

unsafe extern "system" fn target_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: Win32 calls this only for the installed thread-target subclass.
    unsafe { answer(hwnd, message, wparam, lparam, data, false, Some(target_proc)) }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: Win32 calls this only for an installed application-window subclass.
    unsafe { answer(hwnd, message, wparam, lparam, data, true, Some(window_proc)) }
}

#[cfg(test)]
#[path = "windows_session_end_tests.rs"]
mod tests;
