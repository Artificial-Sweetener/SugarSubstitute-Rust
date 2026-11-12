// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native document and timer boundary for the shared Fluent scrollbar.
use super::{
    NativeUiError,
    scrollbar::{Model, Part},
};
use windows::Win32::Foundation::HWND;

/// One native document operation, independent of its text/list/page implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollStep {
    /// Move one document line toward the beginning.
    LineBefore,
    /// Move one document line toward the end.
    LineAfter,
    /// Move one viewport toward the beginning.
    PageBefore,
    /// Move one viewport toward the end.
    PageAfter,
}

/// Page-navigation keys admitted by a scroll viewport, independently of editable text shortcuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollKey {
    /// Move one viewport toward the beginning.
    PageBefore,
    /// Move one viewport toward the end.
    PageAfter,
    /// Move to the first admissible document position.
    Start,
    /// Move to the last admissible document position.
    End,
}
impl ScrollKey {
    /// Interpret only the four Windows viewport-navigation virtual keys.
    pub fn from_virtual_key(key: usize) -> Option<Self> {
        match key {
            0x21 => Some(Self::PageBefore),
            0x22 => Some(Self::PageAfter),
            0x24 => Some(Self::Start),
            0x23 => Some(Self::End),
            _ => None,
        }
    }
}

/// Borrow a retained native scroll document on its creating thread.
///
/// The host owns its window, authoritative position and checked timer inventory.
/// Timer delivery uses that same event loop. No method may wait for background work.
/// Keep this host alive until `ScrollInput::cancel` succeeds before retiring its window.
pub trait ScrollHost {
    /// Original live input window; capture never transfers to a different document.
    fn window(&self) -> HWND;
    /// Monotonic milliseconds used only for the 100-ms visual hover animation.
    fn now_millis(&self) -> u64;
    /// Snapshot document geometry with the supplied shared interaction state.
    /// Return `None` when hidden, closing or without a scrollable range.
    /// # Errors
    /// Preserve native document observation failures.
    fn observe(&self, hover: u8, pressed: Part) -> Result<Option<Model>, NativeUiError>;
    /// Apply one native line/page command without changing text selection.
    /// # Errors
    /// Preserve document operation failures.
    fn scroll(&self, step: ScrollStep) -> Result<(), NativeUiError>;
    /// Scroll signed logical lines, clamped to the document's actual extent.
    /// # Errors
    /// Preserve document observation and scroll failures.
    fn scroll_lines(&self, amount: i64) -> Result<(), NativeUiError>;
    /// Set the vertical document position while preserving its horizontal position.
    /// # Errors
    /// Preserve document observation and scroll failures.
    fn set_position(&self, position: i32) -> Result<(), NativeUiError>;
    /// Observe an actually retained timer obligation before handling its message.
    fn owns_timer(&self, id: usize) -> bool;
    /// Register/update an affine native timer and retain it before accepting success.
    /// # Errors
    /// Preserve native timer admission failures.
    fn set_timer(&self, id: usize, milliseconds: u32) -> Result<(), NativeUiError>;
    /// Retire only this host's timer, retaining failed obligations for retry.
    /// # Errors
    /// Preserve native timer retirement failures.
    fn kill_timer(&self, id: usize) -> Result<(), NativeUiError>;
    /// Request repaint without recursively pumping messages; retain failures at the host.
    fn invalidate(&self);
}
