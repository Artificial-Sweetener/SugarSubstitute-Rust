// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer stable callback data references to and from an owned Win32 subclass.
//!
//! The creating UI thread exclusively installs/removes each procedure/ID pair.
//! Each successful install transfers one Rc clone; explicit removal or the final
//! WM_NCDESTROY callback returns it exactly once. Data must use the same concrete
//! type in both paths. Native final destruction always chains the previous procedure.

use crate::DesktopError;
#[path = "windows_registration.rs"]
mod registration;
pub(crate) use registration::{Registration, callback};
use windows::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Shell::{
            DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SUBCLASSPROC,
            SetWindowSubclass,
        },
        WindowsAndMessaging::WM_NCDESTROY,
    },
};

/// Transfer one stable Rc reference to Win32 only when an absent subclass is registered.
///
/// # Safety
/// The caller owns HWND on this UI thread. The procedure treats transferred data
/// as Rc<T> and uses destroy_callback::<T> exactly once for final WM_NCDESTROY.
pub(crate) unsafe fn install<T>(
    hwnd: HWND,
    procedure: SUBCLASSPROC,
    registration: &Registration<T>,
) -> Result<(), DesktopError> {
    let mut existing = 0;
    // SAFETY: The caller owns the native window on this thread; querying does not retain data.
    if unsafe { GetWindowSubclass(hwnd, procedure, 1, Some(&raw mut existing)) }.as_bool() {
        return Err(DesktopError::message(
            "register_accessible_subclass",
            "native subclass identity is already installed",
        ));
    }
    registration.install(|raw| {
        // SAFETY: Provisional ownership is recorded before native reentry. Failure
        // captures its immediate cause before reclaiming any still-owned reference.
        if !unsafe { SetWindowSubclass(hwnd, procedure, 1, raw) }.as_bool() {
            return Err(bool_failure("register_accessible_subclass"));
        }
        Ok(())
    })
}

/// Reclaim only the confirmed installed data reference belonging to this owner and procedure.
/// A failed removal leaves Win32 owning the reference until its final-destruction callback.
pub(crate) fn remove<T>(
    hwnd: HWND,
    procedure: SUBCLASSPROC,
    registration: &Registration<T>,
) -> Result<(), DesktopError> {
    registration.remove(|expected| {
        let mut existing = 0;
        // SAFETY: Compare identities without dereferencing foreign native data.
        if !unsafe { GetWindowSubclass(hwnd, procedure, 1, Some(&raw mut existing)) }.as_bool() {
            return Err(bool_failure("query_accessible_subclass"));
        }
        if existing != expected {
            return Err(DesktopError::message(
                "remove_accessible_subclass",
                "native registration belongs to another owner",
            ));
        }
        // SAFETY: This exact procedure/data identity is retained on its owning UI thread.
        if !unsafe { RemoveWindowSubclass(hwnd, procedure, 1) }.as_bool() {
            return Err(bool_failure("remove_accessible_subclass"));
        }
        Ok(())
    })
}

fn bool_failure(operation: &'static str) -> DesktopError {
    // SAFETY: Capture the thread's immediate failure fact before cleanup or diagnostics.
    let code = unsafe { GetLastError() }.0;
    if code == 0 {
        DesktopError::message(operation, "native BOOL failed without a Win32 failure code")
    } else {
        DesktopError::new(
            operation,
            windows::core::Error::from(windows::core::HRESULT::from_win32(code)),
        )
    }
}

/// Chain final destruction, then return the still-installed subclass's one native reference.
///
/// # Safety
/// Called only for the installed subclass's final WM_NCDESTROY, with its exact data
/// pointer/type. This reference must not have been returned by explicit removal.
pub(crate) unsafe fn destroy_callback<T>(
    hwnd: HWND,
    procedure: SUBCLASSPROC,
    wparam: WPARAM,
    lparam: LPARAM,
    data: usize,
) -> LRESULT {
    // SAFETY: WM_NCDESTROY is final for this HWND. Its reference stays live through
    // the procedure chain; this UI thread cannot concurrently remove the subclass.
    unsafe {
        let removed = RemoveWindowSubclass(hwnd, procedure, 1).as_bool();
        let code = if removed { 0 } else { GetLastError().0 };
        let result = DefSubclassProc(hwnd, WM_NCDESTROY, wparam, lparam);
        registration::reclaim::<T>(data, true);
        if !removed {
            tracing::warn!(
                operation = "remove_destroyed_native_subclass",
                win32_code = code,
                "Native final-destruction removal was not confirmed before chaining"
            );
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, error::Error, rc::Rc};
    use windows::{
        Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_NULL,
        },
        core::w,
    };

    /// Keep an isolated hidden HWND alive without display, focus or Tao-loop dependencies.
    struct HiddenWindow(HWND);

    impl HiddenWindow {
        /// Create one OS-owned STATIC window local to this test thread.
        fn new() -> Result<Self, Box<dyn Error>> {
            // SAFETY: STATIC is an OS class; no styles make the owned window visible.
            Ok(Self(unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("Native subclass lifetime fixture"),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    1,
                    1,
                    None,
                    None,
                    None,
                    None,
                )?
            }))
        }
    }

    impl Drop for HiddenWindow {
        /// Destroy the fixture synchronously, driving its final native callback on this thread.
        fn drop(&mut self) {
            // SAFETY: This test thread owns the hidden HWND and every installed fixture callback.
            if let Err(error) = unsafe { DestroyWindow(self.0) } {
                tracing::error!(operation = "destroy_native_subclass_fixture", cause = %error, "Native fixture destruction failed");
            }
        }
    }

    /// Observe the actual final native message before using the production release helper.
    unsafe extern "system" fn fixture_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        data: usize,
    ) -> LRESULT {
        if message == WM_NCDESTROY {
            // SAFETY: This installed procedure owns one Rc<Cell<u32>> until final destruction.
            unsafe {
                callback::<Cell<u32>>(data).set(1);
                return destroy_callback::<Cell<u32>>(
                    hwnd,
                    Some(fixture_proc),
                    wparam,
                    lparam,
                    data,
                );
            }
        }
        // SAFETY: Preserve the STATIC procedure and unrelated native messages.
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    /// Use a different procedure identity to force a real failed-removal path.
    unsafe extern "system" fn foreign_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        // SAFETY: The fixture's default native chain accepts these unchanged messages.
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    /// Reenter the native chain during final destruction without touching the removed subclass.
    unsafe extern "system" fn reentrant_chain_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        data: usize,
    ) -> LRESULT {
        // SAFETY: The fixture test retains this cell independently until DestroyWindow returns.
        let state = unsafe { &*(data as *const Cell<u32>) };
        if message == WM_NCDESTROY {
            state.set(state.get() | 2);
            // SAFETY: Sending WM_NULL to this live HWND reenters its installed native chain.
            unsafe {
                SendMessageW(hwnd, WM_NULL, None, None);
            }
        } else if message == WM_NULL {
            state.set(state.get() | 4);
        }
        // SAFETY: Preserve final native destruction and all reentrant messages.
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    /// Returning the installed reference after chaining remains exact under native reentrancy.
    #[test]
    fn native_final_destruction_chains_reentrant_messages_before_releasing_reference()
    -> Result<(), Box<dyn Error>> {
        let window = HiddenWindow::new()?;
        let state = Rc::new(Cell::new(0));
        let registration = Registration::new(&state);
        // SAFETY: The borrowed cell remains live through synchronous fixture destruction;
        // the distinct subclass ID is owned by the OS until that same destruction.
        unsafe {
            SetWindowSubclass(window.0, Some(reentrant_chain_proc), 2, Rc::as_ptr(&state) as usize)
                .ok()?;
        }
        // SAFETY: The fixture HWND belongs to this thread and fixture_proc consumes Rc<Cell<u32>>.
        unsafe {
            install(window.0, Some(fixture_proc), &registration)?;
        }
        assert_eq!(registration.reference_count(), 2);
        drop(window);
        assert_eq!(state.get(), 7);
        assert_eq!(registration.reference_count(), 1);
        Ok(())
    }

    /// Reclaim one installed reference on explicit removal, and never release it again at destruction.
    #[test]
    fn explicit_native_subclass_removal_returns_exactly_one_reference() -> Result<(), Box<dyn Error>>
    {
        let window = HiddenWindow::new()?;
        let state = Rc::new(Cell::new(0));
        let registration = Registration::new(&state);
        // SAFETY: The fixture HWND belongs to this thread and fixture_proc consumes Rc<Cell<u32>>.
        unsafe {
            install(window.0, Some(fixture_proc), &registration)?;
        }
        assert_eq!(registration.reference_count(), 2);
        // SAFETY: The existing fixture procedure uses the same data type and UI thread.
        assert!(unsafe { install(window.0, Some(fixture_proc), &registration) }.is_err());
        assert_eq!(registration.reference_count(), 2);
        assert!(remove(window.0, Some(fixture_proc), &registration).is_ok());
        assert_eq!(registration.reference_count(), 1);
        assert!(remove(window.0, Some(fixture_proc), &registration).is_err());
        drop(window);
        assert_eq!(state.get(), 0);
        assert_eq!(registration.reference_count(), 1);
        Ok(())
    }

    /// A failed removal leaves the reference live until actual native final destruction returns it.
    #[test]
    fn failed_native_removal_returns_reference_on_final_destruction() -> Result<(), Box<dyn Error>>
    {
        let window = HiddenWindow::new()?;
        let state = Rc::new(Cell::new(0));
        let registration = Registration::new(&state);
        // SAFETY: The fixture HWND belongs to this thread and fixture_proc consumes Rc<Cell<u32>>.
        unsafe {
            install(window.0, Some(fixture_proc), &registration)?;
        }
        let foreign_state = Rc::new(Cell::new(0));
        let foreign_registration = Registration::new(&foreign_state);
        assert!(remove(window.0, Some(fixture_proc), &foreign_registration).is_err());
        assert_eq!(foreign_registration.reference_count(), 1);
        assert!(remove(window.0, Some(foreign_proc), &registration).is_err());
        assert_eq!(registration.reference_count(), 2);
        drop(window);
        assert_eq!(state.get(), 1);
        assert_eq!(registration.reference_count(), 1);
        Ok(())
    }

    /// Invalid native registration must immediately return the untransferred callback clone.
    #[test]
    fn failed_native_registration_retains_only_the_owner_reference() {
        let state = Rc::new(Cell::new(0));
        let registration = Registration::new(&state);
        // SAFETY: The invalid HWND cannot install a callback; its expected data type matches.
        assert!(unsafe { install(HWND::default(), Some(fixture_proc), &registration) }.is_err());
        assert_eq!(registration.reference_count(), 1);
        assert_eq!(state.get(), 0);
    }
}
