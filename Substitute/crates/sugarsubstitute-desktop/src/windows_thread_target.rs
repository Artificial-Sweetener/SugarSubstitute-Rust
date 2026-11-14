// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep another program's synchronous repaint from ending a process with a Tao event loop.
//!
//! Tao 0.37 creates a hidden thread-target window per event loop and uses its WM_PAINT only to
//! learn that the message queue is empty: the handler flushes the paint messages of Tao's
//! windows and asserts that no flush is already running. The window is a visible layered popup,
//! so a program that repaints every window synchronously (RedrawWindow or UpdateWindow from
//! another thread) delivers it a sent WM_PAINT. When that send arrives while a flush is peeking
//! messages, from any of the loop's windows, the assertion panics and the process ends.
//!
//! Queued WM_PAINT is never sent, so a sent WM_PAINT carries nothing Tao needs: this subclass
//! validates it and answers without reaching Tao. Everything else passes through unchanged.
//! Install it on the UI thread right after every event loop is built.
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, TRUE, WPARAM},
    Graphics::Gdi::ValidateRect,
    System::Threading::GetCurrentThreadId,
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            EnumThreadWindows, GetClassNameW, ISMEX_NOSEND, InSendMessageEx, WM_NCDESTROY, WM_PAINT,
        },
    },
};
use windows::core::BOOL;

/// The class Tao 0.37 registers for its hidden thread-target window.
const TAO_THREAD_TARGET_CLASS: &str = "Tao Thread Event Target";
/// This subclass's identity among the thread target's subclasses.
const SUBCLASS_ID: usize = 0x5355_4750; // "SUGP"

/// Guard every Tao thread-target window of the calling thread. Installing twice replaces the
/// same subclass, so calling it after each event loop is built is safe.
pub(crate) fn guard_current_thread() {
    for hwnd in thread_windows() {
        if class_name(hwnd).as_deref() == Some(TAO_THREAD_TARGET_CLASS) {
            guard(hwnd);
        }
    }
}

/// Install the guard on one window owned by the calling thread. A window that cannot be
/// subclassed keeps Tao's own behaviour; nothing else depends on the guard.
pub(crate) fn guard(hwnd: HWND) {
    // SAFETY: `hwnd` belongs to the calling thread and the procedure keeps no data.
    let installed = unsafe { SetWindowSubclass(hwnd, Some(procedure), SUBCLASS_ID, 0) };
    if !installed.as_bool() {
        tracing::warn!(
            operation = "thread_target_paint_guard",
            "The event loop's paint guard could not be installed"
        );
    }
}

/// Whether a message with `send` flags is a WM_PAINT another thread sent synchronously.
fn sent_paint(message: u32, send: u32) -> bool {
    message == WM_PAINT && send != ISMEX_NOSEND
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: Reads only the calling thread's current message state.
    if sent_paint(message, unsafe { InSendMessageEx(None) }) {
        // SAFETY: Validating this thread's own window has no other effect.
        let _ = unsafe { ValidateRect(Some(hwnd), None) };
        return LRESULT(0);
    }
    if message == WM_NCDESTROY {
        // SAFETY: The window is being destroyed on its own thread.
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(procedure), SUBCLASS_ID) };
    }
    // SAFETY: Every other message keeps the window's own procedure chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Every top-level window of the calling thread.
fn thread_windows() -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, found: LPARAM) -> BOOL {
        // SAFETY: `found` is the Vec this call lends for the synchronous enumeration only.
        unsafe { (*(found.0 as *mut Vec<HWND>)).push(hwnd) };
        TRUE
    }
    let mut found = Vec::new();
    // SAFETY: The enumeration is synchronous and `found` outlives it.
    let _ = unsafe {
        EnumThreadWindows(GetCurrentThreadId(), Some(collect), LPARAM(&raw mut found as isize))
    };
    found
}

fn class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0_u16; 256];
    // SAFETY: The buffer outlives the call and its length bounds the copy.
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    let length = usize::try_from(length).ok().filter(|length| *length > 0)?;
    String::from_utf16(&buffer[..length]).ok()
}

#[cfg(test)]
#[path = "windows_thread_target_tests.rs"]
mod tests;
