// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain local-document IPC registrations through checked removal on the UI thread.
use crate::{DesktopError, native_action::NativeAction};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};
use tao::event_loop::EventLoopProxy;
use webview2_com::{
    Microsoft::Web::WebView2::Win32::*, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, WebMessageReceivedEventHandler,
};
use windows::{
    Win32::System::Com::CoTaskMemFree,
    core::{HSTRING, PWSTR},
};

struct State {
    active: Cell<bool>,
    document_pending: Cell<bool>,
    queue: RefCell<VecDeque<String>>,
    failure: RefCell<Option<DesktopError>>,
    proxy: EventLoopProxy<NativeAction>,
}
impl State {
    fn fail(&self, cause: DesktopError) {
        if self.failure.borrow().is_none() {
            *self.failure.borrow_mut() = Some(cause);
        }
        self.wake();
    }
    fn wake(&self) {
        if let Err(cause) = self.proxy.send_event(NativeAction::Main) {
            tracing::error!(operation="installer_bridge_wake", cause=%cause, "Installer event loop closed");
        }
    }
}
pub(super) struct Bridge {
    core: ICoreWebView2,
    state: Rc<State>,
    message: Option<i64>,
    navigation: Option<i64>,
    new_window: Option<i64>,
}
impl Bridge {
    pub(super) fn new(core: ICoreWebView2, proxy: EventLoopProxy<NativeAction>) -> Self {
        Self {
            core,
            state: Rc::new(State {
                active: Cell::new(true),
                document_pending: Cell::new(true),
                queue: RefCell::new(VecDeque::new()),
                failure: RefCell::new(None),
                proxy,
            }),
            message: None,
            navigation: None,
            new_window: None,
        }
    }
    pub(super) fn install(&mut self) -> Result<(), DesktopError> {
        let state = self.state.clone();
        let message = WebMessageReceivedEventHandler::create(Box::new(move |_, args| {
            if !state.active.get() {
                return Ok(());
            }
            let result = (|| {
                let args =
                    args.ok_or_else(|| DesktopError::message("installer_ipc", "Event missing"))?;
                let mut source = NativeString(PWSTR::null());
                // SAFETY: Native outputs are immediately retained, bounded and freed on this UI thread.
                unsafe { args.Source(&mut source.0) }
                    .map_err(|e| DesktopError::new("installer_ipc_origin", e))?;
                if source.text(64)? != "about:blank" {
                    return Err(DesktopError::message(
                        "installer_ipc_origin",
                        "Unexpected document",
                    ));
                }
                let mut body = NativeString(PWSTR::null());
                // SAFETY: The event transfers this CoTaskMem string to the retained local owner.
                unsafe { args.TryGetWebMessageAsString(&mut body.0) }
                    .map_err(|e| DesktopError::new("installer_ipc_body", e))?;
                let body = body.text(65536)?;
                if body.len() > 65536 {
                    return Err(DesktopError::message(
                        "installer_ipc_body",
                        "UTF-8 request exceeds bound",
                    ));
                }
                let mut queue = state.queue.borrow_mut();
                if queue.len() >= 32 {
                    return Err(DesktopError::message(
                        "installer_ipc_queue",
                        "Request queue exceeded",
                    ));
                }
                queue.push_back(body);
                Ok(())
            })();
            match result {
                Ok(()) => state.wake(),
                Err(cause) => state.fail(cause),
            }
            Ok(())
        }));
        let mut token = 0;
        // SAFETY: The owner retains the core and adopts each registration token before observing status.
        let result = unsafe { self.core.add_WebMessageReceived(&message, &mut token) };
        self.message = Some(token);
        result.map_err(|e| DesktopError::new("installer_ipc_install", e))?;
        let state = self.state.clone();
        let navigation = NavigationStartingEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                let mut uri = NativeString(PWSTR::null());
                // SAFETY: Valid native event interface and owned output; no queue borrow spans native calls.
                let result = unsafe { args.Uri(&mut uri.0) };
                let blank = uri.text(64).is_ok_and(|uri| uri == "about:blank");
                let inline = uri.starts_with("data:text/html");
                let mut initiated = windows::core::BOOL::default();
                let mut redirected = windows::core::BOOL::default();
                // SAFETY: Scalar native outputs remain local to this callback.
                let native = unsafe {
                    args.IsUserInitiated(&mut initiated)
                        .and_then(|()| args.IsRedirected(&mut redirected))
                };
                let allowed = result.is_ok()
                    && native.is_ok()
                    && state.document_pending.get()
                    && !initiated.as_bool()
                    && !redirected.as_bool()
                    && (blank || inline);
                if allowed && inline {
                    state.document_pending.set(false);
                }
                tracing::info!(
                    operation = "installer_navigation",
                    allowed,
                    blank,
                    inline,
                    "Installer navigation checked"
                );
                // SAFETY: Cancel external navigation on the current event only.
                if let Err(cause) = unsafe { args.SetCancel(!allowed || !state.active.get()) } {
                    state.fail(DesktopError::new("installer_navigation", cause));
                }
            }
            Ok(())
        }));
        let mut token = 0;
        // SAFETY: Same retained registration boundary as above.
        let result = unsafe { self.core.add_NavigationStarting(&navigation, &mut token) };
        self.navigation = Some(token);
        result.map_err(|e| DesktopError::new("installer_navigation_install", e))?;
        let state = self.state.clone();
        let new_window = NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                // SAFETY: This local document never creates additional native windows.
                if let Err(cause) = unsafe { args.SetHandled(true) } {
                    state.fail(DesktopError::new("installer_popup", cause));
                }
            }
            Ok(())
        }));
        let mut token = 0;
        // SAFETY: Adopt the output token even when registration fails.
        let result = unsafe { self.core.add_NewWindowRequested(&new_window, &mut token) };
        self.new_window = Some(token);
        result.map_err(|e| DesktopError::new("installer_popup_install", e))
    }
    pub(super) fn next(&self) -> Result<Option<String>, DesktopError> {
        if let Some(cause) = self.state.failure.borrow_mut().take() {
            return Err(cause);
        }
        Ok(self.state.queue.borrow_mut().pop_front())
    }
    /// Seal initialization once the authored document connects.
    pub(super) fn document_ready(&self) {
        self.state.document_pending.set(false);
    }
    pub(super) fn post(&self, message: &str) -> Result<(), DesktopError> {
        // SAFETY: This bounded string remains live through the UI-affine synchronous call.
        unsafe { self.core.PostWebMessageAsJson(&HSTRING::from(message)) }
            .map_err(|e| DesktopError::new("installer_reply", e))
    }
    pub(super) fn retire(&mut self) -> Result<bool, DesktopError> {
        self.state.active.set(false);
        if let Some(token) = self.message {
            // SAFETY: Remove exactly the retained token; failure keeps the obligation.
            unsafe { self.core.remove_WebMessageReceived(token) }
                .map_err(|e| DesktopError::new("installer_ipc_remove", e))?;
            self.message = None;
        }
        if let Some(token) = self.navigation {
            // SAFETY: This owner still retains the original event source.
            unsafe { self.core.remove_NavigationStarting(token) }
                .map_err(|e| DesktopError::new("installer_navigation_remove", e))?;
            self.navigation = None;
        }
        if let Some(token) = self.new_window {
            // SAFETY: This owner still retains the original event source.
            unsafe { self.core.remove_NewWindowRequested(token) }
                .map_err(|e| DesktopError::new("installer_popup_remove", e))?;
            self.new_window = None;
        }
        Ok(Rc::strong_count(&self.state) == 1)
    }
}
struct NativeString(PWSTR);
impl NativeString {
    fn starts_with(&self, prefix: &str) -> bool {
        if self.0.is_null() {
            return false;
        }
        for (index, expected) in prefix.encode_utf16().enumerate() {
            // SAFETY: Native getters return NUL-terminated UTF-16. A NUL differs
            // from each prefix character, so no read continues beyond it.
            if unsafe { self.0.as_ptr().add(index).read() } != expected {
                return false;
            }
        }
        true
    }
    fn text(&self, limit: usize) -> Result<String, DesktopError> {
        if self.0.is_null() {
            return Err(DesktopError::message("installer_string", "Native string missing"));
        }
        let mut units = Vec::new();
        for index in 0..=limit {
            // SAFETY: A successful WebView2 getter returns NUL-terminated UTF-16; reads stop at NUL or the protocol bound.
            let unit = unsafe { self.0.as_ptr().add(index).read() };
            if unit == 0 {
                return String::from_utf16(&units)
                    .map_err(|e| DesktopError::new("installer_string_unicode", e));
            }
            units.push(unit);
        }
        Err(DesktopError::message("installer_string", "Native string exceeds protocol bound"))
    }
}
impl Drop for NativeString {
    fn drop(&mut self) {
        // SAFETY: Each getter transfers its CoTaskMem allocation, including partial failure output.
        unsafe { CoTaskMemFree(Some(self.0.as_ptr().cast())) };
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        if self.message.is_some() || self.navigation.is_some() || self.new_window.is_some() {
            std::mem::forget(self.core.clone());
            std::mem::forget(self.state.clone());
            tracing::error!(
                operation = "installer_bridge_unsettled",
                "Retaining unresolved document registrations"
            );
        }
    }
}
