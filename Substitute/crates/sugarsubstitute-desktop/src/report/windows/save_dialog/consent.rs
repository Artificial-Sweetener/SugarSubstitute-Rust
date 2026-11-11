// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe explicit overwrite opt-in on the actual modal picker, without pathname probes.
use super::*;
use windows::Win32::{Foundation::RECT, UI::HiDpi::GetDpiForWindow};

/// Retain the native control and label through the same modal parent's checked destruction.
pub(super) struct Consent {
    hwnd: Option<HWND>,
    label: Text,
    lease: MemoryLease,
    selected: Option<bool>,
}
impl Consent {
    /// Default to no consent; only the final native FILEOK observation can supply a value.
    pub(super) fn new(label: Text, lease: &MemoryLease) -> Self {
        Self { hwnd: None, label, lease: lease.clone(), selected: None }
    }
    /// Append one localized checkbox to the real picker after its standard layout is complete.
    pub(super) fn install(&mut self, parent: HWND) -> Result<(), ReportSurfaceError> {
        if self.hwnd.is_some() {
            return Err(ReportSurfaceError::rejected("report_picker_consent_duplicate"));
        }
        let mut client = RECT::default();
        let mut outer = RECT::default();
        // SAFETY: The hook supplies its actual live modal parent on this native UI thread.
        unsafe { GetClientRect(parent, &mut client) }
            .map_err(|cause| ReportSurfaceError::new("report_picker_consent_bounds", cause))?;
        // SAFETY: Inspect only the same owned modal parent's current outer rectangle.
        unsafe { GetWindowRect(parent, &mut outer) }
            .map_err(|cause| ReportSurfaceError::new("report_picker_consent_bounds", cause))?;
        // SAFETY: The native parent owns its established DPI context throughout this hook.
        let dpi = unsafe { GetDpiForWindow(parent) };
        if !(48..=768).contains(&dpi) {
            return Err(ReportSurfaceError::rejected("report_picker_consent_dpi"));
        }
        let scale = |value: u32| ((value * dpi).div_ceil(96)) as i32;
        let margin = scale(12);
        let height = scale(36);
        let width = client.right - client.left - 2 * margin;
        if width <= 0 || client.bottom <= 0 {
            return Err(ReportSurfaceError::rejected("report_picker_consent_bounds"));
        }
        // SAFETY: Parent owns this standard native control; its retained label is terminated.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR(self.label.units.as_ptr()),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WINDOW_STYLE((BS_AUTOCHECKBOX | BS_MULTILINE) as u32),
                margin,
                client.bottom,
                width,
                height,
                Some(parent),
                Some(HMENU(0x6fffusize as *mut c_void)),
                None,
                None,
            )
        }
        .map_err(|cause| ReportSurfaceError::new("report_picker_consent_create", cause))?;
        self.hwnd = Some(hwnd);
        // SAFETY: BM_SETCHECK establishes the unchecked default on this exact owned checkbox.
        let _no_return = unsafe { SendMessageW(hwnd, BM_SETCHECK, Some(WPARAM(0)), None) };
        // SAFETY: Borrow the common dialog's native font; the modal parent outlives its child.
        let font = unsafe { SendMessageW(parent, WM_GETFONT, None, None) };
        if font.0 != 0 {
            // SAFETY: WM_SETFONT borrows the parent's font for this parent's child lifetime only.
            let _no_return = unsafe {
                SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)))
            };
        }
        self.label.verify(hwnd, &self.lease)?;
        // SAFETY: Extend only the actual modal parent; retain native position, focus and z-order.
        unsafe {
            SetWindowPos(
                parent,
                None,
                0,
                0,
                outer.right - outer.left,
                outer.bottom - outer.top + height + margin,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|cause| ReportSurfaceError::new("report_picker_consent_layout", cause))?;
        Ok(())
    }
    /// Latch only the actual checkbox state at the selected pathname's final FILEOK notification.
    pub(super) fn observe_selection(&mut self) -> Result<(), ReportSurfaceError> {
        let hwnd = self
            .hwnd
            .ok_or_else(|| ReportSurfaceError::rejected("report_picker_consent_missing"))?;
        // SAFETY: This same modal hook retains its real checkbox on the creating thread.
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err(ReportSurfaceError::rejected("report_picker_consent_missing"));
        }
        // SAFETY: Read the exact standard checkbox; neither a destination nor a file fact selects it.
        let state = unsafe { SendMessageW(hwnd, BM_GETCHECK, None, None) }.0;
        self.record(state)
    }
    /// Validate the closed native checkbox states; unknown/indeterminate state grants nothing.
    fn record(&mut self, state: isize) -> Result<(), ReportSurfaceError> {
        self.selected = None;
        self.selected = Some(match state {
            0 => false,
            1 => true,
            _ => return Err(ReportSurfaceError::rejected("report_picker_consent_state")),
        });
        Ok(())
    }
    /// Require the final native observation; successful path selection alone grants no overwrite.
    pub(super) fn selected(&self) -> Result<bool, ReportSurfaceError> {
        self.selected
            .ok_or_else(|| ReportSurfaceError::rejected("report_picker_consent_unobserved"))
    }
    /// The modal parent owns destruction; retain original label/credit while its child still exists.
    pub(super) fn retired(&self) -> bool {
        // SAFETY: IsWindow observes only a retained value and never borrows callback/native data.
        self.hwnd.is_none_or(|hwnd| !unsafe { IsWindow(Some(hwnd)) }.as_bool())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    #[test]
    fn successful_path_selection_requires_final_checkbox_observation_and_unchecked_grants_nothing()
    -> Result<(), Box<dyn Error>> {
        let mut state = super::super::tests::state()?.consent.ok_or("consent owner absent")?;
        assert!(state.selected().is_err());
        state.record(0)?;
        assert!(!state.selected()?);
        assert!(state.retired());
        Ok(())
    }
    #[test]
    fn final_native_opt_in_and_opt_out_are_exact_and_invalid_state_revokes_previous_value()
    -> Result<(), Box<dyn Error>> {
        let mut state = super::super::tests::state()?.consent.ok_or("consent owner absent")?;
        state.record(1)?;
        assert!(state.selected()?);
        state.record(0)?;
        assert!(!state.selected()?);
        state.record(1)?;
        assert!(state.record(2).is_err());
        assert!(state.selected().is_err());
        Ok(())
    }
}
