// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exactly one callback-data reference per installed native subclass.
//!
//! Subclasses are installed/removed only on their creating UI thread. Each
//! successful install transfers one Rc clone to Win32; explicit successful removal
//! or WM_NCDESTROY returns that clone exactly once. Unrelated procedures remain chained.

use crate::{
    DesktopError,
    native_action::NativeAction,
    windows_subclass::{callback, destroy_callback},
};
use std::cell::{Cell, RefCell};
#[path = "windows_caption.rs"]
pub(crate) mod caption;
#[path = "windows_selection.rs"]
mod selection;
#[path = "timing_native_callback.rs"]
pub(crate) mod timing;
use crate::timing::{NativeCallbackKind, TimingObserver};
use tao::event_loop::EventLoopProxy;
use timing::NativeCallbackObservation;
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::ValidateRect,
    UI::{
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            HTTRANSPARENT, SC_MOVE, WM_COMMAND, WM_ENTERSIZEMOVE, WM_ERASEBKGND, WM_EXITSIZEMOVE,
            WM_KEYDOWN, WM_KILLFOCUS, WM_NCDESTROY, WM_NCHITTEST, WM_PAINT, WM_SETFOCUS,
            WM_SYSCOMMAND,
        },
    },
};

/// Actual input owner behind a prepared native peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputKind {
    Prepared,
    Text,
    Select,
}

/// Stable child identity and the current typed action retained by its parent callback.
struct Registration {
    hwnd: HWND,
    id: u16,
    action: Option<NativeAction>,
    input: InputKind,
    selection: sugarsubstitute_ui::form::selection::SelectionGesture,
}

/// Keep immutable peer identities and the UI-only wake proxy behind stable reference ownership.
pub(crate) struct Callback {
    proxy: EventLoopProxy<NativeAction>,
    timings: Option<TimingObserver>,
    controls: RefCell<Vec<Registration>>,
    failure: RefCell<Option<DesktopError>>,
    paint_mode: crate::windows_controls::PaintMode,
    pub(crate) caption: Cell<Option<caption::Caption>>,
    pub(crate) hovered: Cell<HWND>,
}

impl Callback {
    /// Start an empty native inventory before constructing child HWNDs.
    pub(crate) fn new(
        proxy: EventLoopProxy<NativeAction>,
        timings: Option<TimingObserver>,
        paint_mode: crate::windows_controls::PaintMode,
        count: usize,
    ) -> Result<Self, DesktopError> {
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(count)
            .map_err(|cause| DesktopError::new("allocate_native_identities", cause))?;
        if controls.capacity() != count {
            return Err(DesktopError::message(
                "allocate_native_identities",
                "identity capacity exceeded",
            ));
        }
        Ok(Self {
            proxy,
            timings,
            controls: RefCell::new(controls),
            failure: RefCell::new(None),
            paint_mode,
            caption: Cell::new(None),
            hovered: Cell::new(HWND::default()),
        })
    }
    pub(crate) fn proxy(&self) -> EventLoopProxy<NativeAction> {
        self.proxy.clone()
    }
    pub(crate) fn timings(&self) -> Option<TimingObserver> {
        self.timings.clone()
    }
    pub(crate) fn prepared_pixels(&self) -> bool {
        self.paint_mode == crate::windows_controls::PaintMode::PreparedPixels
    }

    /// Register one created peer; no callback borrow is held across native construction.
    pub(crate) fn register(
        &self,
        hwnd: HWND,
        id: u16,
        action: Option<NativeAction>,
        input: InputKind,
    ) -> Result<(), DesktopError> {
        let mut controls = self
            .controls
            .try_borrow_mut()
            .map_err(|error| DesktopError::new("register_accessible_identity", error))?;
        if controls.len() == controls.capacity() {
            return Err(DesktopError::message(
                "register_accessible_identity",
                "identity capacity exhausted",
            ));
        }
        controls.push(Registration { hwnd, id, action, input, selection: Default::default() });
        Ok(())
    }

    /// Account retained child identities before any native registration.
    pub(crate) fn storage_bytes(count: usize) -> Option<usize> {
        count
            .checked_mul(size_of::<Registration>())?
            .checked_add(size_of::<Self>() + 2 * size_of::<usize>())
    }
    /// Replace one already-registered peer mapping; no borrow crosses a native call.
    pub(crate) fn set_action(
        &self,
        hwnd: HWND,
        id: u16,
        action: Option<NativeAction>,
    ) -> Result<(), DesktopError> {
        let mut controls = self
            .controls
            .try_borrow_mut()
            .map_err(|cause| DesktopError::new("update_native_action", cause))?;
        let slot = controls.iter_mut().find(|peer| peer.hwnd == hwnd && peer.id == id).ok_or_else(
            || DesktopError::message("update_native_action", "retained peer missing"),
        )?;
        slot.action = action;
        Ok(())
    }

    /// Resolve only a registered peer HWND and, for parent notifications, the matching control ID.
    fn action(&self, hwnd: HWND, id: Option<u16>) -> Option<NativeAction> {
        match self.controls.try_borrow() {
            Ok(controls) => controls
                .iter()
                .find(|peer| peer.hwnd == hwnd && id.is_none_or(|id| id == peer.id))
                .and_then(|peer| peer.action),
            Err(error) => {
                tracing::error!(operation = "resolve_accessible_identity", cause = %error, "Native identity inventory was unexpectedly borrowed");
                None
            }
        }
    }

    /// Resolve only a registered child's actual native input implementation.
    fn input(&self, hwnd: HWND, id: Option<u16>) -> Option<InputKind> {
        match self.controls.try_borrow() {
            Ok(controls) => controls
                .iter()
                .find(|peer| peer.hwnd == hwnd && id.is_none_or(|id| id == peer.id))
                .map(|peer| peer.input),
            Err(cause) => {
                tracing::error!(operation = "native_control_input", cause = %cause, "Native input inventory was borrowed");
                None
            }
        }
    }

    /// Wake the native loop without borrowing content or running application work inside a procedure.
    fn send(&self, action: NativeAction) {
        if let Err(error) = self.proxy.send_event(action) {
            tracing::error!(operation = "native_control_delivery", cause = %error, "Native control event could not reach its event loop");
        }
    }
}

/// Route only registered child commands and preserve Tao's native procedure chain.
pub(crate) unsafe extern "system" fn parent_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: Each installed subclass owns its transferred reference through this invocation.
    let (timings, proxy) = {
        // SAFETY: Installed data is live; this short borrow ends before final destruction or forwarding.
        let callback = unsafe { callback::<Callback>(data) };
        (callback.timings.clone(), callback.proxy.clone())
    };
    let mut observation =
        NativeCallbackObservation::new(timings, proxy, NativeCallbackKind::WindowsParent);
    let result = (|| {
        if message == WM_NCDESTROY {
            // SAFETY: This callback still owns its transferred Rc until this final message.
            return unsafe {
                destroy_callback::<Callback>(hwnd, Some(parent_proc), wparam, lparam, data)
            };
        }
        // SAFETY: The nonfinal invocation still owns its installed reference.
        let callback = unsafe { callback::<Callback>(data) };
        if let Some(result) = caption::parent(callback, hwnd, message, wparam, lparam) {
            return result;
        }
        if message == windows::Win32::UI::WindowsAndMessaging::WM_CTLCOLORLISTBOX {
            match callback.popup_colors(
                HWND(lparam.0 as *mut std::ffi::c_void),
                windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut std::ffi::c_void),
            ) {
                Ok(Some(brush)) => return LRESULT(brush.0 as isize),
                Ok(None) => {}
                Err(cause) => callback.fail(cause),
            }
        }
        if message == WM_COMMAND
            && lparam.0 != 0
            && (wparam.0 >> 16) == 0
            && let Some(action) = callback
                .action(HWND(lparam.0 as *mut std::ffi::c_void), Some((wparam.0 & 0xffff) as u16))
        {
            callback.send(action);
            return LRESULT(0);
        }
        if message == WM_COMMAND && lparam.0 != 0 {
            use windows::Win32::UI::WindowsAndMessaging::EN_CHANGE;
            let peer = HWND(lparam.0 as *mut std::ffi::c_void);
            let id = (wparam.0 & 0xffff) as u16;
            if callback.input(peer, Some(id)) == Some(InputKind::Select) {
                match selection::notification(callback, peer, wparam.0 >> 16) {
                    Ok(Some(commit)) => {
                        if commit && let Some(action) = callback.action(peer, Some(id)) {
                            callback.send(action);
                        }
                        return LRESULT(0);
                    }
                    Ok(None) => {}
                    Err(cause) => {
                        callback.fail(cause);
                        return LRESULT(0);
                    }
                }
            } else if callback.input(peer, Some(id)) == Some(InputKind::Text)
                && wparam.0 >> 16 == EN_CHANGE as usize
                && let Some(action) = callback.action(peer, Some(id))
            {
                callback.send(action);
                return LRESULT(0);
            }
        }
        if let Some(phase) = move_trace_phase(message, wparam) {
            tracing::info!(
                operation = "native_caption_move_observed",
                phase,
                "Observed owned parent native movement message"
            );
        }
        // SAFETY: Preserve Tao's procedure and every unrelated native subclass.
        if message == WM_SYSCOMMAND && (wparam.0 & 0xfff0) == SC_MOVE as usize {
            observation.delegated(|| {
                // SAFETY: Forward this live owned HWND and unchanged message through its subclass chain.
                unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
            })
        } else {
            // SAFETY: Forward this live owned HWND and unchanged message through its subclass chain.
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
    })();
    observation.finish();
    result
}

/// Identify native movement diagnostics without retaining data across default-procedure reentry.
/// The owned surface is nonresizable; move-loop entry/exit refer to that same parent window.
fn move_trace_phase(message: u32, wparam: WPARAM) -> Option<&'static str> {
    match message {
        WM_SYSCOMMAND if (wparam.0 & 0xfff0) == SC_MOVE as usize => Some("command"),
        WM_ENTERSIZEMOVE => Some("entered"),
        WM_EXITSIZEMOVE => Some("exited"),
        _ => None,
    }
}

/// Retain OS accessibility/Space activation while the content paints and owns pointer behavior.
pub(crate) unsafe extern "system" fn child_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: Each installed subclass owns its transferred reference through this invocation.
    let (timings, proxy) = {
        // SAFETY: Installed data is live; this short borrow ends before final destruction or forwarding.
        let callback = unsafe { callback::<Callback>(data) };
        (callback.timings.clone(), callback.proxy.clone())
    };
    let observation =
        NativeCallbackObservation::new(timings, proxy, NativeCallbackKind::WindowsChild);
    let result = (|| {
        // SAFETY: The installed reference remains live through hover retirement and forwarding.
        let callback = unsafe { callback::<Callback>(data) };
        caption::child(callback, hwnd, message, wparam, lparam);
        if message == WM_NCDESTROY {
            // SAFETY: This callback still owns its transferred Rc until this final message.
            return unsafe {
                destroy_callback::<Callback>(hwnd, Some(child_proc), wparam, lparam, data)
            };
        }
        match message {
            windows::Win32::UI::WindowsAndMessaging::WM_CHAR
                if callback.input(hwnd, None) == Some(InputKind::Text)
                    && matches!(wparam.0, 9 | 13 | 27) =>
            {
                return LRESULT(0);
            }
            WM_NCHITTEST if callback.input(hwnd, None) == Some(InputKind::Text) => {
                return LRESULT(windows::Win32::UI::WindowsAndMessaging::HTCLIENT as isize);
            }
            WM_NCHITTEST
                if callback.prepared_pixels()
                    && callback.input(hwnd, None) == Some(InputKind::Prepared) =>
            {
                return LRESULT(HTTRANSPARENT as isize);
            }
            WM_ERASEBKGND if callback.prepared_pixels() || callback.caption.get().is_some() => {
                return LRESULT(1);
            }
            WM_PAINT
                if callback.prepared_pixels()
                    && callback.input(hwnd, None) != Some(InputKind::Text) =>
            {
                // SAFETY: Validate this child while the content paints its control rectangle.
                if !unsafe { ValidateRect(Some(hwnd), None) }.as_bool() {
                    tracing::error!(
                        operation = "validate_accessible_control_paint",
                        "ValidateRect failed"
                    );
                }
                return LRESULT(0);
            }
            WM_SETFOCUS | WM_KILLFOCUS if callback.prepared_pixels() => {
                callback.send(NativeAction::FocusChanged)
            }
            WM_KEYDOWN if callback.prepared_pixels() && callback.action(hwnd, None).is_some() => {
                if callback.input(hwnd, None) == Some(InputKind::Prepared)
                    && let Some(key) =
                        sugarsubstitute_ui::windows::scroll_port::ScrollKey::from_virtual_key(
                            wparam.0,
                        )
                {
                    callback.send(NativeAction::ScrollPage(key));
                    return LRESULT(0);
                }
                if callback.input(hwnd, None) == Some(InputKind::Select)
                    && matches!(wparam.0, 0x1b | 0x0d)
                {
                    // SAFETY: Query only this retained native combo box's actual popup state.
                    let expanded = unsafe {
                        windows::Win32::UI::WindowsAndMessaging::SendMessageW(
                            hwnd,
                            windows::Win32::UI::WindowsAndMessaging::CB_GETDROPPEDSTATE,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        )
                    }
                    .0 != 0;
                    if expanded {
                        // SAFETY: Let the original combo box close/accept its own popup.
                        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
                    }
                }
                match wparam.0 {
                    0x1b => {
                        callback.send(NativeAction::Close);
                        return LRESULT(0);
                    }
                    0x09 => {
                        callback.send(NativeAction::FocusNext(
                            crate::windows_controls::shift_pressed(),
                        ));
                        return LRESULT(0);
                    }
                    0x0d if lparam.0 & (1 << 30) == 0
                        && callback.input(hwnd, None) == Some(InputKind::Prepared) =>
                    {
                        if let Some(action) = callback.action(hwnd, None) {
                            callback.send(action);
                        }
                        return LRESULT(0);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        // SAFETY: Preserve the original text engine, accessibility and native button procedures.
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    })();
    observation.finish();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{HTCAPTION, SC_CLOSE, SC_SIZE};

    /// Trace both caption and keyboard move commands while leaving unrelated traffic silent.
    #[test]
    fn native_move_trace_filters_commands_without_changing_native_parameters() {
        assert_eq!(move_trace_phase(WM_SYSCOMMAND, WPARAM(SC_MOVE as usize)), Some("command"));
        assert_eq!(
            move_trace_phase(WM_SYSCOMMAND, WPARAM((SC_MOVE | HTCAPTION) as usize)),
            Some("command")
        );
        assert_eq!(move_trace_phase(WM_ENTERSIZEMOVE, WPARAM(0)), Some("entered"));
        assert_eq!(move_trace_phase(WM_EXITSIZEMOVE, WPARAM(0)), Some("exited"));
        for command in [SC_CLOSE, SC_SIZE] {
            assert_eq!(move_trace_phase(WM_SYSCOMMAND, WPARAM(command as usize)), None);
        }
        assert_eq!(move_trace_phase(WM_COMMAND, WPARAM(SC_MOVE as usize)), None);
        assert_eq!(move_trace_phase(WM_PAINT, WPARAM(0)), None);
    }
}
