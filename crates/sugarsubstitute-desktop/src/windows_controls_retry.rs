// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Apply one preallocated native retry peer without lending callback borrows across Win32 reentry.
use super::*;
use crate::main_backend::{RetryAction, RetryPresentation};
use windows::Win32::Foundation::{ERROR_SUCCESS, GetLastError, SetLastError};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
    WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, IsWindow, SWP_HIDEWINDOW, SWP_NOMOVE, SWP_NOSIZE,
        SWP_SHOWWINDOW,
    },
};

/// Hidden snapshots and the Main retry peer begin as disabled, nonvisible children.
pub(super) fn initial_style(mode: PaintMode, id: u16, visible: bool) -> WINDOW_STYLE {
    WS_CHILD
        | WS_TABSTOP
        | WINDOW_STYLE(BS_PUSHBUTTON as u32)
        | if !visible || matches!(mode, PaintMode::NativeButtons(_)) && id == 4 {
            WS_DISABLED
        } else {
            WS_VISIBLE
        }
}

/// External native effects only; the production transition and ordering stay in apply.
trait RetryPort {
    fn live(&self) -> Result<(), DesktopError>;
    fn action(&mut self, action: Option<RetryAction>) -> Result<(), DesktopError>;
    fn enabled(&mut self, enabled: bool) -> Result<(), DesktopError>;
    fn label(&mut self, label: &'static str) -> Result<(), DesktopError>;
    fn visible(&mut self, visible: bool) -> Result<(), DesktopError>;
}
fn apply(port: &mut impl RetryPort, presentation: RetryPresentation) -> Result<(), DesktopError> {
    if presentation
        .action
        .is_some_and(|action| !presentation.visible || action.identity != presentation.identity)
    {
        return Err(DesktopError::message(
            "present_retry",
            "native action and presentation identity differ",
        ));
    }
    port.live()?;
    port.action(None)?;
    port.enabled(false)?;
    port.live()?;
    port.label(presentation.label)?;
    port.live()?;
    port.visible(presentation.visible)?;
    port.live()?;
    port.enabled(presentation.action.is_some())?;
    port.live()?;
    port.action(presentation.action)
}
impl Controls {
    /// Change only id4 after exact Main matching; all native peers keep their original identity.
    pub(crate) fn present_retry(
        &mut self,
        presentation: RetryPresentation,
    ) -> Result<(), DesktopError> {
        if self.paint_mode != PaintMode::NativeButtons(presentation.identity) {
            return Err(DesktopError::message("present_retry", "current Main identity differs"));
        }
        if !self.creation.writable() {
            if !presentation.visible && presentation.action.is_none() {
                return Ok(());
            }
            return Err(DesktopError::message(
                "present_retry",
                "original control owner is not writable",
            ));
        }
        let peer =
            self.creation.peers.iter_mut().find(|slot| slot.value.snapshot.id == 4).ok_or_else(
                || DesktopError::message("present_retry", "preallocated native retry peer missing"),
            )?;
        {
            let mut port = WindowRetry {
                callback: &self.callback,
                peer: &peer.value,
                parent: self.creation.parent.as_ref(),
            };
            apply(&mut port, presentation)?;
        }
        peer.value.snapshot.label = presentation.label;
        peer.value.snapshot.visible = presentation.visible;
        Ok(())
    }
}
struct WindowRetry<'a> {
    callback: &'a Callback,
    peer: &'a Peer,
    parent: Option<&'a Registration<Callback>>,
}
impl RetryPort for WindowRetry<'_> {
    fn live(&self) -> Result<(), DesktopError> {
        if self.parent.is_none_or(|parent| !parent.installed() || parent.destroyed())
            || self
                .peer
                .registration
                .as_ref()
                .is_none_or(|peer| !peer.installed() || peer.destroyed())
        {
            return Err(DesktopError::message(
                "present_retry",
                "original callback registration retired",
            ));
        }
        // SAFETY: This creating thread retains the exact registration through the query.
        if !unsafe { IsWindow(Some(self.peer.hwnd)) }.as_bool() {
            return Err(DesktopError::message("present_retry", "original native peer retired"));
        }
        Ok(())
    }
    fn action(&mut self, action: Option<RetryAction>) -> Result<(), DesktopError> {
        self.callback.set_action(self.peer.hwnd, 4, action.map(NativeAction::MainBackendRetry))
    }
    fn enabled(&mut self, enabled: bool) -> Result<(), DesktopError> {
        // SAFETY: Owner retains this thread-affine peer and registrations. The return
        // describes previous enabled state, not success; query current state afterward.
        let _previous = unsafe { EnableWindow(self.peer.hwnd, enabled) };
        self.live()?;
        // SAFETY: Revalidated original peer; observation never grants backend authority.
        if unsafe { IsWindowEnabled(self.peer.hwnd) }.as_bool() != enabled {
            return Err(DesktopError::message("enable_retry", "native enabled state differs"));
        }
        Ok(())
    }
    fn label(&mut self, label: &'static str) -> Result<(), DesktopError> {
        let mut text = [0_u16; 129];
        for (count, unit) in label.encode_utf16().enumerate() {
            if count >= text.len() - 1 {
                return Err(DesktopError::message("label_retry", "admitted label bound exceeded"));
            }
            text[count] = unit;
        }
        // SAFETY: Exact retained peer; Win32 synchronously copies this bounded NUL-terminated stack label.
        unsafe { SetWindowTextW(self.peer.hwnd, PCWSTR(text.as_ptr())) }
            .map_err(|cause| DesktopError::new("label_retry", cause))
    }
    fn visible(&mut self, visible: bool) -> Result<(), DesktopError> {
        let visibility = if visible { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
        // SAFETY: Creating thread retains the original peer; no focus, reorder or geometry change.
        unsafe {
            SetWindowPos(
                self.peer.hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | visibility,
            )
        }
        .map_err(|cause| DesktopError::new("show_retry", cause))?;
        self.live()?;
        // SAFETY: Retained owned BUTTON always has a nonzero child style. Observe its
        // own WS_VISIBLE bit; a hidden parent must not be mistaken for a hidden peer.
        let (style, code) = unsafe {
            SetLastError(ERROR_SUCCESS);
            let style = GetWindowLongPtrW(self.peer.hwnd, GWL_STYLE) as u32;
            (style, GetLastError())
        };
        if style == 0 && code != ERROR_SUCCESS {
            return Err(DesktopError::new(
                "query_retry_visibility",
                windows::core::Error::from(windows::core::HRESULT::from_win32(code.0)),
            ));
        }
        if (style & WS_VISIBLE.0 != 0) != visible || style & WS_CHILD.0 == 0 {
            return Err(DesktopError::message(
                "show_retry",
                "native visibility or child style differs",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "windows_controls_retry_tests.rs"]
mod tests;
