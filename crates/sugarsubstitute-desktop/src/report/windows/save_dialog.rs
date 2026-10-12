// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one modal HWND timer and one deferred update until checked hook retirement.
use super::super::{
    ReportSessionContent, ReportSurfaceError, ReportSurfaceUpdate, SaveSelection, state::Request,
};
use super::buffer::Text;
use std::{ffi::c_void, num::NonZeroUsize};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    incident::ExportSelection,
    localization::incident::ReportMessages,
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::{Controls::Dialogs::*, WindowsAndMessaging::*},
    },
    core::{PCWSTR, PWSTR, w},
};

type Poller = unsafe fn(*mut c_void) -> Result<ReportSurfaceUpdate, ReportSurfaceError>;
struct Hook {
    context: *mut c_void,
    poller: Option<Poller>,
    active: bool,
    initialized: bool,
    installed: bool,
    destroyed: bool,
    timer: usize,
    update: Option<ReportSurfaceUpdate>,
    failure: Option<ReportSurfaceError>,
    save: Text,
    cancel_label: Text,
}
struct Dialog {
    hook: Box<Hook>,
    options: Box<OPENFILENAMEW>,
    _title: Text,
    _filter: Vec<u16>,
    destination: Vec<u16>,
    _filter_claim: MemoryClaim,
    claim: MemoryClaim,
}
impl Hook {
    fn retired(&self) -> bool {
        !self.active && (!self.initialized || (self.installed && self.destroyed && self.timer == 0))
    }
    fn begin_poll(&mut self) -> Result<Option<(Poller, *mut c_void)>, ReportSurfaceError> {
        if self.active {
            return Err(ReportSurfaceError::rejected("report_picker_recursive_poll"));
        }
        if self.update.is_some() || self.failure.is_some() {
            return Ok(None);
        }
        let Some(poller) = self.poller else {
            return Ok(None);
        };
        self.active = true;
        Ok(Some((poller, self.context)))
    }
    fn complete_poll(&mut self, result: Result<ReportSurfaceUpdate, ReportSurfaceError>) -> bool {
        self.active = false;
        match result {
            Ok(ReportSurfaceUpdate::None) => false,
            Ok(update) => {
                self.update = Some(update);
                true
            }
            Err(cause) => {
                self.fail(cause);
                true
            }
        }
    }
    fn fail(&mut self, cause: ReportSurfaceError) {
        if self.failure.is_none() {
            self.failure = Some(cause);
        }
    }
    fn cancel(&mut self, hwnd: HWND) {
        // SAFETY: Only the actual hook's retained parent modal dialog is messaged.
        let result = unsafe {
            GetParent(hwnd)
                .and_then(|parent| PostMessageW(Some(parent), WM_COMMAND, WPARAM(2), LPARAM(0)))
        };
        if let Err(cause) = result {
            self.fail(ReportSurfaceError::new("report_picker_cancel", cause));
        }
    }
}
unsafe fn poll<T: ReportSessionContent>(
    context: *mut c_void,
) -> Result<ReportSurfaceUpdate, ReportSurfaceError> {
    // SAFETY: Native modal call retains the exclusive T borrow; recursion is rejected before invocation.
    unsafe {
        (&mut *context.cast::<T>()).poll(
            NonZeroUsize::new(32)
                .ok_or_else(|| ReportSurfaceError::rejected("report_picker_budget"))?,
        )
    }
}
unsafe extern "system" fn hook(hwnd: HWND, message: u32, _wparam: WPARAM, lparam: LPARAM) -> usize {
    if message == WM_INITDIALOG {
        // SAFETY: Common dialog supplies its live OPENFILENAMEW during WM_INITDIALOG.
        let options = unsafe { &*(lparam.0 as *const OPENFILENAMEW) };
        // SAFETY: lCustData is our retained Box<Hook> for this synchronous modal call.
        let state = unsafe { &mut *(options.lCustData.0 as *mut Hook) };
        state.initialized = true;
        // SAFETY: Hook child HWND belongs exclusively to this dialog; retained heap pointer stays live.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, options.lCustData.0);
        }
        // SAFETY: Check the actual private hook registration before admitting a timer or borrow.
        if unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } != options.lCustData.0 {
            state.fail(ReportSurfaceError::new(
                "report_picker_hook_register",
                windows::core::Error::from_thread(),
            ));
            state.cancel(hwnd);
            return 0;
        }
        state.installed = true;
        // SAFETY: One HWND-owned UI timer, with no thread-pool callback or detached work.
        state.timer = unsafe { SetTimer(Some(hwnd), 1, 16, None) };
        if state.timer == 0 {
            state.fail(ReportSurfaceError::new(
                "report_picker_timer",
                windows::core::Error::from_thread(),
            ));
            state.cancel(hwnd);
        }
        return 0;
    }
    // SAFETY: Read only our private hook dialog's stable registration.
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Hook;
    if pointer.is_null() {
        return 0;
    }
    // SAFETY: Installed pointer remains boxed until actual WM_NCDESTROY/timer retirement.
    let state = unsafe { &mut *pointer };
    if message == WM_NOTIFY {
        // SAFETY: Common dialog owns the bounded notification during this hook invocation.
        let notification = unsafe { &*(lparam.0 as *const OFNOTIFYW) };
        if notification.hdr.code == CDN_INITDONE {
            let save = state.save.units.as_ptr();
            let cancel = state.cancel_label.units.as_ptr();
            let _ = state;
            // SAFETY: Retained terminated strings remain live through synchronous native dispatch.
            let result = unsafe {
                GetParent(hwnd).and_then(|parent| {
                    SetDlgItemTextW(parent, 1, PCWSTR(save))?;
                    SetDlgItemTextW(parent, 2, PCWSTR(cancel))
                })
            };
            if let Err(cause) = result {
                // SAFETY: Native dispatch completed and the hook box remains owned.
                let state = unsafe { &mut *pointer };
                state.fail(ReportSurfaceError::new("report_picker_localization", cause));
                state.cancel(hwnd);
            }
            return 0;
        }
    }
    if message == WM_TIMER {
        let invocation = match state.begin_poll() {
            Ok(invocation) => invocation,
            Err(cause) => {
                state.fail(cause);
                state.cancel(hwnd);
                return 0;
            }
        };
        if let Some((poller, context)) = invocation {
            // End the exclusive Hook borrow before calling content, which can reenter native dispatch.
            let _ = state;
            // SAFETY: Exclusive callback borrow is active only inside this synchronous modal call.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                poller(context)
            }));
            // SAFETY: The retained hook box outlives nested dispatch and the modal call.
            let state = unsafe { &mut *pointer };
            let result = result.unwrap_or_else(|payload| {
                Err(ReportSurfaceError::new(
                    "report_picker_poll_panicked",
                    super::super::error::PanicCause::new(payload),
                ))
            });
            if state.complete_poll(result) {
                state.cancel(hwnd);
            }
        }
    }
    // SAFETY: No external content call holds an exclusive reference across this borrow.
    let state = unsafe { &mut *pointer };
    if message == WM_NCDESTROY {
        if state.timer != 0 {
            // SAFETY: Retire exactly this hook HWND's single owned timer before callback storage release.
            match unsafe { KillTimer(Some(hwnd), state.timer) } {
                Ok(()) => state.timer = 0,
                Err(cause) => {
                    state.fail(ReportSurfaceError::new("report_picker_timer_retire", cause))
                }
            }
        }
        // SAFETY: Actual final destroy discharges this native pointer registration.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        state.destroyed = true;
    }
    0
}
mod retirement;
mod selection;
#[cfg(test)]
mod tests;
pub(super) use retirement::UnretiredDialog;
pub(super) use selection::select;
