// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Translate original selector notifications through the shared popup-gesture owner.
use super::*;
use sugarsubstitute_ui::form::selection::{SelectionEffect, SelectionEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CB_GETCURSEL, CB_SETCURSEL, CBN_DROPDOWN, CBN_SELCHANGE, CBN_SELENDCANCEL, CBN_SELENDOK,
    SendMessageW,
};
impl Callback {
    /// Match the original registered combo's popup before applying shared native colors.
    pub(super) fn popup_colors(
        &self,
        popup: HWND,
        dc: windows::Win32::Graphics::Gdi::HDC,
    ) -> Result<Option<windows::Win32::Graphics::Gdi::HBRUSH>, DesktopError> {
        let mut index = 0;
        loop {
            let peer = {
                let peers = self
                    .controls
                    .try_borrow()
                    .map_err(|cause| DesktopError::new("native_select_popup", cause))?;
                peers.get(index).map(|peer| (peer.hwnd, peer.input))
            };
            let Some((hwnd, input)) = peer else {
                return Ok(None);
            };
            if input == InputKind::Select
                && sugarsubstitute_ui::windows::select::popup(hwnd)? == popup
            {
                return Ok(Some(sugarsubstitute_ui::windows::select::popup_colors(dc)?));
            }
            index += 1;
        }
    }

    /// Release the registry borrow before executing any native effect or synchronous notification.
    fn selection(
        &self,
        hwnd: HWND,
        event: SelectionEvent,
    ) -> Result<SelectionEffect, DesktopError> {
        let mut controls = self
            .controls
            .try_borrow_mut()
            .map_err(|e| DesktopError::new("native_select_state", e))?;
        let peer = controls
            .iter_mut()
            .find(|peer| peer.hwnd == hwnd && peer.input == InputKind::Select)
            .ok_or_else(|| {
                DesktopError::message("native_select_state", "Original selector absent")
            })?;
        Ok(peer.selection.observe(event))
    }
    /// Read only the original popup's committed selection without borrowing across native effects.
    pub(crate) fn selection_origin(
        &self,
        hwnd: HWND,
    ) -> Result<Option<Option<usize>>, DesktopError> {
        let controls =
            self.controls.try_borrow().map_err(|e| DesktopError::new("native_select_origin", e))?;
        Ok(controls
            .iter()
            .find(|peer| peer.hwnd == hwnd && peer.input == InputKind::Select)
            .ok_or_else(|| {
                DesktopError::message("native_select_origin", "Original selector absent")
            })?
            .selection
            .original())
    }
    /// Invalidate a local popup preview before authoritative options or selection are replaced.
    pub(crate) fn replace_selection(&self, hwnd: HWND) -> Result<(), DesktopError> {
        self.selection(hwnd, SelectionEvent::Replace)?;
        Ok(())
    }
    /// Preserve the first callback failure for the native event loop's normal checked cleanup.
    pub(super) fn fail(&self, cause: DesktopError) {
        if self.failure.borrow().is_none() {
            *self.failure.borrow_mut() = Some(cause);
        }
        self.send(NativeAction::FocusChanged);
    }
    /// Transfer a callback error once; the caller must enter its retained cleanup transaction.
    pub(crate) fn check(&self) -> Result<(), DesktopError> {
        self.failure.borrow_mut().take().map_or(Ok(()), Err)
    }
}
/// Return None for unrelated selector notifications; never run application work inside the callback.
pub(super) fn notification(
    callback: &Callback,
    hwnd: HWND,
    notification: usize,
) -> Result<Option<bool>, DesktopError> {
    let event = match notification as u32 {
        CBN_DROPDOWN => {
            // SAFETY: Parent dispatch already verified this retained original selector and its ID.
            let index = unsafe { SendMessageW(hwnd, CB_GETCURSEL, None, None) }.0;
            if index < -1 {
                return Err(DesktopError::message(
                    "native_select_begin",
                    "Invalid native selection",
                ));
            }
            SelectionEvent::Open((index >= 0).then_some(index as usize))
        }
        CBN_SELCHANGE => SelectionEvent::Changed,
        CBN_SELENDOK => SelectionEvent::Accept,
        CBN_SELENDCANCEL => SelectionEvent::Cancel,
        _ => return Ok(None),
    };
    match callback.selection(hwnd, event)? {
        SelectionEffect::None => Ok(Some(false)),
        SelectionEffect::Commit => Ok(Some(true)),
        SelectionEffect::Restore(index) => {
            let expected = index.map_or(-1, |index| index as isize);
            // SAFETY: Restore only the original list while no callback registry borrow is held.
            let actual = unsafe {
                SendMessageW(hwnd, CB_SETCURSEL, Some(WPARAM(expected as usize)), None);
                SendMessageW(hwnd, CB_GETCURSEL, None, None).0
            };
            if actual != expected {
                return Err(DesktopError::message(
                    "native_select_cancel",
                    "Original selection restoration failed",
                ));
            }
            Ok(Some(false))
        }
    }
}
