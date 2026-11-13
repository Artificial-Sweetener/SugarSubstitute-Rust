// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a field's native caret without retiring another field's queue-owned caret.
use super::NativeUiError;
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::HBITMAP,
    System::Threading::GetCurrentThreadId,
    UI::{Input::KeyboardAndMouse::GetFocus, WindowsAndMessaging::*},
};

/// Affine caret state borrowing a retained window. Call `close` before destroying that window.
/// The native queue has one caret; a later focused field may replace this one's caret.
pub struct Caret {
    hwnd: HWND,
    created: Cell<bool>,
    visible: Cell<bool>,
    affinity: PhantomData<Rc<()>>,
}

impl Caret {
    /// Construct inert state. No native caret is acquired until this window has actual focus.
    pub fn new(hwnd: HWND) -> Self {
        Self { hwnd, created: Cell::new(false), visible: Cell::new(false), affinity: PhantomData }
    }

    /// Create the engine-provided shape on the owning window thread, only while focused.
    /// The caller retains any non-null bitmap until replacement or checked closure.
    /// # Errors
    /// Rejects invalid dimensions, foreign-thread windows and native creation failures.
    pub fn create(&self, bitmap: HBITMAP, width: i32, height: i32) -> Result<(), NativeUiError> {
        self.thread()?;
        if width < 0 || height < 0 {
            return Err(NativeUiError::message("text_caret_shape", "Invalid caret dimensions"));
        }
        // SAFETY: Observe only this thread's focus; never acquire focus or activate a window.
        if unsafe { GetFocus() } != self.hwnd {
            return Ok(());
        }
        // SAFETY: The caller retains the affine HWND and the engine-provided bitmap.
        unsafe { CreateCaret(self.hwnd, (!bitmap.is_invalid()).then_some(bitmap), width, height) }
            .map_err(|cause| NativeUiError::new("text_caret_create", cause))?;
        self.created.set(true);
        self.visible.set(false);
        Ok(())
    }

    /// Apply a visibility state without accumulating Windows' ShowCaret/HideCaret reference count.
    /// # Errors
    /// Preserves current-thread ownership and native visibility failures.
    pub fn show(&self, visible: bool) -> Result<(), NativeUiError> {
        if self.current()? && self.visible.get() != visible {
            // SAFETY: The current queue caret was created for this exact retained affine window.
            unsafe {
                if visible { ShowCaret(Some(self.hwnd)) } else { HideCaret(Some(self.hwnd)) }
            }
            .map_err(|cause| NativeUiError::new("text_caret_visibility", cause))?;
            self.visible.set(visible);
        }
        Ok(())
    }

    /// Move this field's caret in native client coordinates, without touching a replacement caret.
    /// # Errors
    /// Preserves current-thread ownership and native positioning failures.
    pub fn position(&self, x: i32, y: i32) -> Result<(), NativeUiError> {
        if self.current()? {
            // SAFETY: The observed queue caret belongs to this exact retained affine window.
            unsafe { SetCaretPos(x, y) }
                .map_err(|cause| NativeUiError::new("text_caret_position", cause))?;
        }
        Ok(())
    }

    /// Retire only a caret still owned by this field; safe after another field has taken focus.
    /// # Errors
    /// Preserves current-thread ownership and native retirement failures for a later retry.
    pub fn close(&self) -> Result<(), NativeUiError> {
        if self.current()? {
            // SAFETY: No reentrant native call occurs between identity observation and retirement.
            unsafe { DestroyCaret() }
                .map_err(|cause| NativeUiError::new("text_caret_retire", cause))?;
        }
        self.created.set(false);
        self.visible.set(false);
        Ok(())
    }

    fn current(&self) -> Result<bool, NativeUiError> {
        if !self.created.get() {
            return Ok(false);
        }
        let thread = self.thread()?;
        let mut info =
            GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        // SAFETY: Inspect this exact owning thread, never the user's foreground thread.
        unsafe { GetGUIThreadInfo(thread, &mut info) }
            .map_err(|cause| NativeUiError::new("text_caret_identity", cause))?;
        Ok(info.hwndCaret == self.hwnd)
    }

    fn thread(&self) -> Result<u32, NativeUiError> {
        // SAFETY: Passive native identity queries do not retain or mutate the borrowed window.
        let (owner, current) =
            unsafe { (GetWindowThreadProcessId(self.hwnd, None), GetCurrentThreadId()) };
        if owner == 0 || owner != current {
            return Err(NativeUiError::message(
                "text_caret_thread",
                "Caret window is not owned by this thread",
            ));
        }
        Ok(owner)
    }
}
