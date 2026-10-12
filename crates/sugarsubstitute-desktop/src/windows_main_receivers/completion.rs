// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound raw CDP completion text before allocation and retain real callback obligations.

use crate::{
    DesktopError,
    main_events::{Command, Events, copy_utf16},
};
use std::{cell::RefCell, rc::Rc};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{HRESULT, HSTRING, PCWSTR, implement};

pub(super) struct Operations {
    pending: RefCell<Vec<u64>>,
    next: RefCell<u64>,
    events: Rc<Events>,
}
impl Operations {
    pub(super) fn new(events: Rc<Events>) -> Result<Rc<Self>, DesktopError> {
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(4)
            .map_err(|cause| DesktopError::new("allocate_main_commands", cause))?;
        if pending.capacity() != 4 {
            return Err(DesktopError::message(
                "allocate_main_commands",
                "command capacity exceeded",
            ));
        }
        Ok(Rc::new(Self { pending: RefCell::new(pending), next: RefCell::new(1), events }))
    }
    fn arm(&self) -> Result<u64, DesktopError> {
        let mut pending = self.pending.borrow_mut();
        if pending.len() == 4 {
            return Err(DesktopError::message("submit_main_command", "command capacity exhausted"));
        }
        let mut next = self.next.borrow_mut();
        let id = *next;
        *next = id.checked_add(1).ok_or_else(|| {
            DesktopError::message("submit_main_command", "command identity exhausted")
        })?;
        pending.push(id);
        Ok(id)
    }
    fn completed(&self, id: u64) {
        let mut pending = self.pending.borrow_mut();
        if let Some(index) = pending.iter().position(|&value| value == id) {
            pending.swap_remove(index);
        } else {
            drop(pending);
            self.events.fail(DesktopError::message(
                "complete_main_command",
                "duplicate or foreign native completion",
            ));
        }
    }
    pub(super) fn pending(&self) -> usize {
        self.pending.borrow().len()
    }
}

#[implement(ICoreWebView2CallDevToolsProtocolMethodCompletedHandler)]
struct Completed {
    owner: Rc<Operations>,
    id: u64,
    kind: Command,
}
impl ICoreWebView2CallDevToolsProtocolMethodCompletedHandler_Impl for Completed_Impl {
    fn Invoke(&self, errorcode: HRESULT, result: &PCWSTR) -> windows::core::Result<()> {
        let _callback = self.owner.events.callback();
        if let Err(cause) = errorcode.ok() {
            self.owner.events.fail(DesktopError::new("main_cdp_completion", cause));
        } else if result.is_null() {
            self.owner.events.fail(DesktopError::new(
                "main_cdp_completion",
                windows::core::Error::from(windows::Win32::Foundation::E_POINTER),
            ));
        } else {
            self.owner.events.record_command(self.kind, |target| {
                copy_utf16(target, |index| {
                    // SAFETY: WebView2 retains a valid borrowed NUL string throughout Invoke.
                    // The scan is bounded before any authored copy; this callback never frees it.
                    unsafe { result.as_ptr().add(index).read() }
                })
            });
        }
        self.owner.completed(self.id);
        Ok(())
    }
}

pub(super) fn call(
    core: &ICoreWebView2,
    owner: &Rc<Operations>,
    kind: Command,
    method: &str,
    parameters: &str,
) -> Result<(), DesktopError> {
    if method.len() > 128
        || parameters.len() > 8192
        || method.contains('\0')
        || parameters.contains('\0')
    {
        return Err(DesktopError::message("submit_main_command", "command input bound exceeded"));
    }
    // HSTRING copies are bounded and covered by the caller's retained receiver metadata claim.
    let method = HSTRING::from(method);
    let parameters = HSTRING::from(parameters);
    let id = owner.arm()?;
    let handler: ICoreWebView2CallDevToolsProtocolMethodCompletedHandler =
        Completed { owner: owner.clone(), id, kind }.into();
    // SAFETY: The current UI thread owns core/handler. Arm before a possibly synchronous
    // callback. A failed submission cannot fabricate callback quiescence: its slot stays owned.
    unsafe { core.CallDevToolsProtocolMethod(&method, &parameters, &handler) }
        .map_err(|cause| DesktopError::new("submit_main_command", cause))
}
