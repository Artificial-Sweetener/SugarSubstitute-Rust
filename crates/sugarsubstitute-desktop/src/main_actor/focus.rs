// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep keyboard focus with the main browser across clicks, activation and Tab traversal.
//!
//! WebView2 receives keyboard and IME input through its own focus window beneath the
//! controller's parent, never through the host window, so the host passes those messages
//! through untouched. Windows gives an activated top-level window the focus itself; the host
//! therefore moves focus into the browser whenever the main window receives it, for windowed
//! and composed hosting alike. A windowed browser takes focus from its own clicks; a composed
//! browser receives forwarded presses, so the host also moves focus before forwarding a press
//! while the browser lacks it. `MoveFocus` with the programmatic reason restores the page's
//! previously focused element. The main window has no other keyboard stop, so Tab traversal
//! leaving the page wraps to the page's first or last element.
use crate::DesktopError;
use std::{cell::Cell, rc::Rc};
use webview2_com::{
    FocusChangedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_MOVE_FOCUS_REASON, COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT,
        COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS, COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC,
        ICoreWebView2Controller, ICoreWebView2MoveFocusRequestedEventArgs,
    },
    MoveFocusRequestedEventHandler,
};
use windows::Win32::Foundation::E_POINTER;

/// Whether an admitted pointer transition needs to release page text-entry focus.
///
/// Forwarded pointer input lets the page choose its own focused control. A captured contact
/// whose press never reaches the page must release the previous text-entry owner itself.
#[cfg(feature = "native-composition")]
pub(in crate::main_actor) fn releases_page_text_entry(pressed: bool, forwarded: bool) -> bool {
    pressed && !forwarded
}

/// Whether forwarding this mouse message must first move keyboard focus into the browser.
///
/// A press moves focus only while the browser reports that it lacks focus, so clicks inside an
/// already focused page keep the page's own focus handling. The host window itself receiving
/// `WM_SETFOCUS` always passes focus on, whatever WebView2 last reported.
#[cfg(feature = "native-composition")]
pub(in crate::main_actor) fn requests_focus(message: u32, browser_focused: bool) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_RBUTTONDBLCLK,
        WM_RBUTTONDOWN, WM_XBUTTONDBLCLK, WM_XBUTTONDOWN,
    };
    match message {
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_MBUTTONDOWN | WM_MBUTTONDBLCLK | WM_RBUTTONDOWN
        | WM_RBUTTONDBLCLK | WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => !browser_focused,
        _ => false,
    }
}

/// Where Tab traversal leaving the page continues: forward wraps to the page's first element
/// and backward to its last. Any other reason keeps WebView2's default handling.
pub(in crate::main_actor) fn wrap(
    reason: COREWEBVIEW2_MOVE_FOCUS_REASON,
) -> Option<COREWEBVIEW2_MOVE_FOCUS_REASON> {
    (reason == COREWEBVIEW2_MOVE_FOCUS_REASON_NEXT
        || reason == COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS)
        .then_some(reason)
}

/// The browser's reported focus and the exact controller registrations that report it.
///
/// UI-thread affine. Handlers retain only the shared focus flag, never the host state, and
/// every token is removed before the controller is released.
pub(in crate::main_actor) struct Focus {
    controller: ICoreWebView2Controller,
    focused: Rc<Cell<bool>>,
    got: Cell<Option<i64>>,
    lost: Cell<Option<i64>>,
    traversal: Cell<Option<i64>>,
}

impl Focus {
    pub(in crate::main_actor) fn new(controller: ICoreWebView2Controller) -> Self {
        Self {
            controller,
            focused: Rc::new(Cell::new(false)),
            got: Cell::new(None),
            lost: Cell::new(None),
            traversal: Cell::new(None),
        }
    }

    /// Whether WebView2 last reported that the browser holds keyboard focus.
    #[cfg(feature = "native-composition")]
    pub(in crate::main_actor) fn browser_focused(&self) -> bool {
        self.focused.get()
    }

    /// Register focus reporting and Tab wrapping once; returned tokens are retained for
    /// removal even when registration reports failure.
    pub(in crate::main_actor) fn install(&self) -> Result<(), DesktopError> {
        let controller = &self.controller;
        for (slot, focused) in [(&self.got, true), (&self.lost, false)] {
            let flag = self.focused.clone();
            let handler = FocusChangedEventHandler::create(Box::new(move |_, _| {
                flag.set(focused);
                Ok(())
            }));
            register(slot, |token| {
                // SAFETY: The handler retains only a UI-thread flag; the token output is local.
                unsafe {
                    if focused {
                        controller.add_GotFocus(&handler, token)
                    } else {
                        controller.add_LostFocus(&handler, token)
                    }
                }
            })?;
        }
        let handler = MoveFocusRequestedEventHandler::create(Box::new(|sender, arguments| {
            if let Err(cause) = wrap_traversal(sender, arguments) {
                tracing::warn!(operation = "wrap_composed_browser_focus", cause = %cause,
                    "Tab traversal leaving the page keeps WebView2's default handling");
            }
            Ok(())
        }));
        register(&self.traversal, |token| {
            // SAFETY: The handler retains no host state; the token output is live local storage.
            unsafe { controller.add_MoveFocusRequested(&handler, token) }
        })
    }

    /// Move keyboard focus into the browser, restoring the page's previously focused element.
    ///
    /// Failure is logged rather than escalated: focus is advisory, the window stays usable, and
    /// the next activation or press retries.
    pub(in crate::main_actor) fn enter(&self) {
        // SAFETY: The retained controller belongs to this UI thread.
        let moved =
            unsafe { self.controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) };
        if let Err(cause) = moved {
            tracing::warn!(operation = "focus_composed_browser", cause = %cause,
                "Keyboard focus remains outside the composed browser");
        }
    }

    /// Release the previous page control after a captured pen contact whose press was withheld
    /// from WebView2. Forwarded mouse presses leave focus with the page's pointer handler.
    ///
    /// Failure is logged rather than escalated: focus is advisory and the next press retries.
    #[cfg(feature = "native-composition")]
    pub(in crate::main_actor) fn release_captured_pen_focus(&self) {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2ExecuteScriptCompletedHandler as Completed;
        // SAFETY: The retained controller belongs to this UI thread; the script is a constant
        // and its completion is not observed.
        let moved = unsafe {
            self.controller.CoreWebView2().and_then(|browser| {
                browser.ExecuteScript(
                    windows::core::w!("document.activeElement?.blur?.()"),
                    None::<&Completed>,
                )
            })
        };
        if let Err(cause) = moved {
            tracing::warn!(operation = "focus_composed_page", cause = %cause,
                "Page focus stays where it was before the canvas press");
        }
    }

    /// Remove every registration before the controller is released; failures remain retryable.
    pub(in crate::main_actor) fn retire(&self) -> Result<(), DesktopError> {
        let controller = &self.controller;
        // SAFETY: Each exact token belongs to this retained controller on its UI thread.
        unregister(&self.got, |token| unsafe { controller.remove_GotFocus(token) })?;
        // SAFETY: As above.
        unregister(&self.lost, |token| unsafe { controller.remove_LostFocus(token) })?;
        // SAFETY: As above.
        unregister(&self.traversal, |token| unsafe { controller.remove_MoveFocusRequested(token) })
    }
}

/// Claim Tab traversal leaving the page and continue it inside the page.
fn wrap_traversal(
    sender: Option<ICoreWebView2Controller>,
    arguments: Option<ICoreWebView2MoveFocusRequestedEventArgs>,
) -> windows::core::Result<()> {
    let (Some(sender), Some(arguments)) = (sender, arguments) else {
        return Err(windows::core::Error::from(E_POINTER));
    };
    let mut reason = COREWEBVIEW2_MOVE_FOCUS_REASON::default();
    // SAFETY: The event's live arguments copy one scalar into local storage.
    unsafe { arguments.Reason(&raw mut reason)? };
    if let Some(wrapped) = wrap(reason) {
        // SAFETY: The sender is the live controller raising this event on its UI thread.
        unsafe {
            arguments.SetHandled(true)?;
            sender.MoveFocus(wrapped)?;
        }
    }
    Ok(())
}

fn register(
    slot: &Cell<Option<i64>>,
    add: impl FnOnce(*mut i64) -> windows::core::Result<()>,
) -> Result<(), DesktopError> {
    if slot.get().is_some() {
        return Ok(());
    }
    let mut token = 0;
    let result = add(&raw mut token);
    if result.is_ok() || token != 0 {
        slot.set(Some(token));
    }
    result.map_err(|cause| DesktopError::new("install_browser_focus_events", cause))
}

fn unregister(
    slot: &Cell<Option<i64>>,
    remove: impl FnOnce(i64) -> windows::core::Result<()>,
) -> Result<(), DesktopError> {
    if let Some(token) = slot.get() {
        remove(token).map_err(|cause| DesktopError::new("remove_browser_focus_events", cause))?;
        slot.set(None);
    }
    Ok(())
}

#[cfg(test)]
#[path = "focus/tests.rs"]
mod tests;
