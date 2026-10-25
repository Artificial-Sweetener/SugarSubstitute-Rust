// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Typed synchronous RichEdit messages; no raw pointer/message port escapes this adapter.
use crate::{DesktopError, console::index::TextStream};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        Controls::{EM_GETFIRSTVISIBLELINE, EM_LINEINDEX, EM_LINESCROLL, RichEdit::*},
        WindowsAndMessaging::{SB_BOTTOM, SendMessageW, WM_VSCROLL},
    },
};
/// Borrow only the original live affine peer retained by the containing Console.
pub(super) struct Messages(pub(super) HWND);
/// Keep mutually exclusive native character-formatting scopes explicit at the boundary.
pub(super) enum FormatScope {
    /// Set the insertion defaults retained by the original native document.
    Default,
    /// Override only the exact selected source style run.
    Selection,
}
impl Messages {
    /// Read native selection endpoints in internal UTF-16 character units.
    pub(super) fn selection(&self) -> CHARRANGE {
        let mut range = CHARRANGE::default();
        // SAFETY: Original live peer synchronously writes the documented stack output.
        unsafe {
            SendMessageW(self.0, EM_EXGETSEL, None, Some(LPARAM((&raw mut range) as isize)));
        }
        range
    }
    /// Set and verify an exact nonnegative range; -1 selects the native end sentinel.
    pub(super) fn select(&self, start: i32, end: i32) -> Result<(), DesktopError> {
        let range = CHARRANGE { cpMin: start, cpMax: end };
        // SAFETY: The exact affine peer synchronously reads this typed live range.
        let result = unsafe {
            SendMessageW(self.0, EM_EXSETSEL, None, Some(LPARAM((&raw const range) as isize))).0
        };
        let actual = self.selection();
        if start >= 0 && actual.cpMin != start || end >= 0 && actual.cpMax != end {
            return Err(DesktopError::message(
                "console_selection_restore",
                format!(
                    "requested ({start}, {end}), observed ({}, {}), native result {result}",
                    actual.cpMin, actual.cpMax
                ),
            ));
        }
        Ok(())
    }
    /// Read the first visible wrapped display-line index without changing focus.
    pub(super) fn first_line(&self) -> Result<i32, DesktopError> {
        // SAFETY: Scalar query on the original live affine peer.
        let result = unsafe { SendMessageW(self.0, EM_GETFIRSTVISIBLELINE, None, None).0 };
        i32::try_from(result).map_err(|e| DesktopError::new("console_first_line", e))
    }
    /// Resolve an observed display line to its first UTF-16 character.
    pub(super) fn line_index(&self, line: i32) -> Result<i32, DesktopError> {
        if line < 0 {
            return Err(rejected("console_line_index"));
        }
        // SAFETY: Scalar display-line index supplied by this same peer.
        let result =
            unsafe { SendMessageW(self.0, EM_LINEINDEX, Some(WPARAM(line as usize)), None).0 };
        i32::try_from(result).map_err(|e| DesktopError::new("console_line_index", e))
    }
    /// Resolve a matching document character to its wrapped display line.
    pub(super) fn line_from_character(&self, position: i32) -> Result<i32, DesktopError> {
        if position < 0 {
            return Err(rejected("console_line_from_character"));
        }
        // SAFETY: Scalar character position derived from the retained matching document.
        let result = unsafe {
            SendMessageW(self.0, EM_EXLINEFROMCHAR, None, Some(LPARAM(position as isize))).0
        };
        i32::try_from(result).map_err(|e| DesktopError::new("console_line_from_character", e))
    }
    /// Scroll vertically; retain a failed multiline-control response as an error.
    pub(super) fn scroll_lines(&self, delta: i32) -> Result<(), DesktopError> {
        if delta == 0 {
            return Ok(());
        }
        // SAFETY: Synchronous scalar vertical scroll on the retained multiline peer.
        if unsafe { SendMessageW(self.0, EM_LINESCROLL, None, Some(LPARAM(delta as isize))).0 } == 0
        {
            return Err(rejected("console_scroll_lines"));
        }
        Ok(())
    }
    /// Follow the document tail even when the read-only console has no active caret or focus.
    pub(super) fn scroll_bottom(&self) -> Result<(), DesktopError> {
        crate::windows_text::find(self.0)?.message(
            WM_VSCROLL,
            WPARAM(SB_BOTTOM.0 as usize),
            LPARAM(0),
        )?;
        Ok(())
    }
    /// Copy the complete already-prepared Unicode byte stream synchronously.
    pub(super) fn stream(
        &self,
        reader: &mut TextStream<'_>,
        selection: bool,
    ) -> Result<(), DesktopError> {
        let mut transfer = EDITSTREAM {
            dwCookie: (reader as *mut TextStream<'_>) as usize,
            dwError: 0,
            pfnCallback: Some(stream),
        };
        // SAFETY: Streaming is synchronous. The reader and transfer remain borrowed through
        // every callback; the callback only copies prepared bytes and cannot call providers.
        unsafe {
            SendMessageW(
                self.0,
                EM_STREAMIN,
                Some(WPARAM(
                    (SF_TEXT | SF_UNICODE | if selection { SFF_SELECTION } else { 0 }) as usize,
                )),
                Some(LPARAM((&raw mut transfer) as isize)),
            );
        }
        let error = transfer.dwError;
        if error != 0 {
            return Err(DesktopError::new(
                "console_text_stream",
                windows::core::Error::from(windows::core::HRESULT::from_win32(error)),
            ));
        }
        if !reader.complete() {
            return Err(rejected("console_text_stream"));
        }
        Ok(())
    }
    /// Read the exact native UTF-16 count excluding the terminal NUL.
    pub(super) fn length(&self) -> isize {
        let facts = GETTEXTLENGTHEX { flags: GTL_NUMCHARS | GTL_PRECISE, codepage: 1200 };
        // SAFETY: Typed immutable request lives through the synchronous Unicode query.
        unsafe {
            SendMessageW(
                self.0,
                EM_GETTEXTLENGTHEX,
                Some(WPARAM((&raw const facts) as usize)),
                None,
            )
            .0
        }
    }
    /// Read exact raw characters into a separately admitted nonempty UTF-16 buffer.
    pub(super) fn read(&self, buffer: &mut [u16]) -> Result<usize, DesktopError> {
        if buffer.is_empty() {
            return Err(rejected("console_text_readback"));
        }
        let bytes = buffer
            .len()
            .checked_mul(2)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| rejected("console_text_readback"))?;
        let facts =
            GETTEXTEX { cb: bytes, flags: GT_RAWTEXT, codepage: 1200, ..Default::default() };
        // SAFETY: Buffer has the exact declared writable byte extent and remains borrowed.
        let length = unsafe {
            SendMessageW(
                self.0,
                EM_GETTEXTEX,
                Some(WPARAM((&raw const facts) as usize)),
                Some(LPARAM(buffer.as_mut_ptr() as isize)),
            )
            .0
        };
        usize::try_from(length)
            .ok()
            .filter(|n| *n < buffer.len())
            .ok_or_else(|| rejected("console_text_readback"))
    }
    /// Apply initialized character formatting to its one documented native scope.
    pub(super) fn format(
        &self,
        scope: FormatScope,
        format: &CHARFORMAT2W,
    ) -> Result<(), DesktopError> {
        let scope = match scope {
            FormatScope::Default => SCF_DEFAULT,
            FormatScope::Selection => SCF_SELECTION | SCF_NOKBUPDATE,
        };
        // SAFETY: Exact initialized CHARFORMAT2W remains live during synchronous copying.
        if unsafe {
            SendMessageW(
                self.0,
                EM_SETCHARFORMAT,
                Some(WPARAM(scope as usize)),
                Some(LPARAM((format as *const CHARFORMAT2W) as isize)),
            )
            .0
        } == 0
        {
            return Err(rejected("console_text_format"));
        }
        Ok(())
    }
    #[cfg(test)]
    /// Inspect selected formatting on the actual hidden native fixture.
    pub(super) fn selected_format(&self) -> CHARFORMAT2W {
        let mut format = CHARFORMAT2W::default();
        format.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
        // SAFETY: Hidden-peer fixture queries into the exact initialized native output.
        unsafe {
            SendMessageW(
                self.0,
                EM_GETCHARFORMAT,
                Some(WPARAM(SCF_SELECTION as usize)),
                Some(LPARAM((&raw mut format) as isize)),
            );
        }
        format
    }
}
fn rejected(operation: &'static str) -> DesktopError {
    DesktopError::message(operation, "native console message rejected")
}
unsafe extern "system" fn stream(
    cookie: usize,
    buffer: *mut u8,
    capacity: i32,
    written: *mut i32,
) -> u32 {
    if cookie == 0 || buffer.is_null() || written.is_null() || capacity < 0 {
        return 87;
    }
    // SAFETY: Win32 lends these documented writable buffers. Synchronous stream() retains
    // the concrete stack reader; copying the prepared slice invokes no arbitrary code.
    unsafe {
        *written = (&mut *(cookie as *mut TextStream<'_>))
            .read(std::slice::from_raw_parts_mut(buffer, capacity as usize))
            as i32;
    }
    0
}
