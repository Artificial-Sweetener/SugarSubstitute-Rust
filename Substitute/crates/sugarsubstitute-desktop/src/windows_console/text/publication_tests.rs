// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the real RichEdit message boundary while its parent remains hidden.
use super::*;
use crate::windows_subclass::{Registration, callback, destroy_callback, install, remove};
use std::{cell::Cell, rc::Rc};
use windows::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::DefSubclassProc,
        WindowsAndMessaging::{
            GWL_STYLE, GetWindowLongPtrW, IsWindowVisible, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
            SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos, WM_NCDESTROY, WM_SETREDRAW, WS_VISIBLE,
        },
    },
};

/// Track actual native publication effects without substituting for RichEdit behavior.
#[derive(Default)]
struct Publication {
    suspended: Cell<bool>,
    effects: Cell<usize>,
    exposed: Cell<usize>,
    reject_format: Cell<bool>,
}

/// Every temporary stream, formatting selection and final selection stays unpaintable.
/// A rejected native format must also restore redraw without showing the hidden parent.
pub(super) fn publication_never_paints_intermediate_text_or_selection(
    apartment: &UiApartment,
) -> TestResult {
    for reject in [false, true] {
        let (document, _frame, _release) = index(&source(1, &[(1, "visible 日本😀 text")])?)?;
        let mut owner = Hidden::new(document, apartment)?;
        let hwnd = owner.hwnd.ok_or("publication native child absent")?;
        let state = Rc::new(Publication::default());
        state.reject_format.set(reject);
        let registration = Registration::new(&state);
        // SAFETY: Exact hidden-parent child, affine registration and callback share this STA.
        unsafe { install(hwnd, Some(observe), &registration) }?;
        // SAFETY: Make only the child style visible; its retained parent stays hidden.
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )
        }?;
        let document = owner.document.take().ok_or("publication document absent")?;
        let units = document.units as i32;
        let mut documents = update::Documents::new(document);
        let result = publish(hwnd, &mut documents, true);
        let selection = NativeText { hwnd, dark: true }.control().selection();
        // SAFETY: Passive style/visibility observations of the original hidden-parent child.
        let (style, visible) =
            unsafe { (GetWindowLongPtrW(hwnd, GWL_STYLE) as u32, IsWindowVisible(hwnd).as_bool()) };
        remove(hwnd, Some(observe), &registration)?;
        owner.close()?;
        assert!(!visible);
        assert_ne!(style & WS_VISIBLE.0, 0, "publication changed child availability");
        assert!(state.effects.get() > 0, "native text messages did not run");
        assert_eq!(state.exposed.get(), 0, "temporary text/selection was paintable");
        assert!(!state.suspended.get(), "publication left native redraw disabled");
        if reject {
            assert!(result.is_err());
            assert!(documents.pending());
        } else {
            result?;
            assert_eq!((selection.cpMin, selection.cpMax), (units, units));
            assert!(!documents.pending());
        }
    }
    Ok(())
}

/// # Safety
/// Installed only on the original test-thread child with one retained Publication registration.
unsafe extern "system" fn observe(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        // SAFETY: Final destruction returns this exact subclass's native reference.
        return unsafe {
            destroy_callback::<Publication>(hwnd, Some(observe), wparam, lparam, data)
        };
    }
    // SAFETY: The installed registration retains its concrete state through this invocation.
    let state = unsafe { callback::<Publication>(data) };
    if message == WM_SETREDRAW {
        state.suspended.set(wparam.0 == 0);
    }
    if matches!(message, EM_STREAMIN | EM_EXSETSEL | EM_SETCHARFORMAT) {
        state.effects.set(state.effects.get() + 1);
        if !state.suspended.get() {
            state.exposed.set(state.exposed.get() + 1);
        }
    }
    if message == EM_SETCHARFORMAT && state.reject_format.get() {
        return LRESULT(0);
    }
    // SAFETY: Preserve real RichEdit streaming, formatting, selection and redraw semantics.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
