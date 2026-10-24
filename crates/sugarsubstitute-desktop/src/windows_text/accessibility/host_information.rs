// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native RichEdit range geometry reads the same HWND through its documented host information ABI.
use super::*;
use windows::{
    Win32::{
        Foundation::{E_POINTER, S_FALSE, S_OK},
        UI::Accessibility::{UIA_E_ELEMENTNOTAVAILABLE, UiaRect},
    },
    core::{HRESULT, IUnknown, IUnknown_Vtbl, interface},
};

/// SDK textserv.h's interface identity is exported as GUID zero in generated Windows metadata.
#[interface("23969a9d-8546-4032-a1bb-73750cbf3333")]
pub(super) unsafe trait Information: IUnknown {
    fn get_boundary_rectangle(&self, out: *mut UiaRect) -> HRESULT;
    fn is_visible(&self) -> HRESULT;
}
impl Information_Impl for TextHost_Impl {
    unsafe fn get_boundary_rectangle(&self, out: *mut UiaRect) -> HRESULT {
        if out.is_null() {
            return E_POINTER;
        }
        // SAFETY: The documented synchronous callback lends one writable rectangle output.
        unsafe {
            out.write(UiaRect::default());
        }
        if self.state.painting.get() || self.state.closing.get() {
            return HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32);
        }
        let mut rect = RECT::default();
        // SAFETY: The engine retains this exact affine child through all native provider calls.
        let result = unsafe { GetWindowRect(self.state.hwnd, &mut rect) };
        match result {
            Ok(()) => {
                // SAFETY: The initialized ABI output receives current physical screen coordinates.
                unsafe {
                    out.write(UiaRect {
                        left: f64::from(rect.left),
                        top: f64::from(rect.top),
                        width: f64::from(rect.right - rect.left),
                        height: f64::from(rect.bottom - rect.top),
                    });
                }
                S_OK
            }
            Err(cause) => cause.into(),
        }
    }
    unsafe fn is_visible(&self) -> HRESULT {
        if self.state.painting.get() || self.state.closing.get() {
            return HRESULT(UIA_E_ELEMENTNOTAVAILABLE as i32);
        }
        // SAFETY: The native visibility fact belongs to this retained affine child.
        if unsafe { IsWindowVisible(self.state.hwnd) }.as_bool() { S_OK } else { S_FALSE }
    }
}
