// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stream Unicode plain text and apply the shared ANSI palette to the exact held peer.
use crate::{
    DesktopError,
    console::{
        index::{DocumentIndex, TextStream},
        update::{self, Observation, TextPort},
    },
};
use sugarsubstitute_contracts::{configuration::Theme, presentation::console_color};
#[path = "text/redraw.rs"]
mod redraw;
use windows::Win32::{
    Foundation::{COLORREF, HWND},
    UI::Controls::RichEdit::*,
};
/// Publish only a fully copied and formatted retained document.
pub(super) fn publish(
    hwnd: HWND,
    documents: &mut update::Documents,
    dark: bool,
) -> Result<(), DesktopError> {
    if !documents.pending() {
        return Ok(());
    }
    let _redraw = redraw::Suspension::new(hwnd)?;
    let mut port = NativeText { hwnd, dark };
    documents.publish(&mut port)
}
/// Observe only after exact document publication, before a native geometry change.
pub(super) fn observe(hwnd: HWND, dark: bool) -> Result<Observation, DesktopError> {
    NativeText { hwnd, dark }.observe()
}
/// Preserve current selection/tail policy when wrapping or visible row capacity changes.
pub(super) fn reflow(
    hwnd: HWND,
    document: &DocumentIndex,
    observation: Observation,
    dark: bool,
) -> Result<(), DesktopError> {
    let _redraw = redraw::Suspension::new(hwnd)?;
    let follow = observation.at_bottom && observation.start == observation.end;
    let (start, end) = if follow {
        (document.units as i32, document.units as i32)
    } else {
        (observation.start, observation.end)
    };
    NativeText { hwnd, dark }.restore(start, end, observation.first, follow)
}
struct NativeText {
    hwnd: HWND,
    dark: bool,
}
impl NativeText {
    fn control(&self) -> super::messages::Messages {
        super::messages::Messages(self.hwnd)
    }
    fn selection(&self, start: i32, end: i32) -> Result<(), DesktopError> {
        self.control().select(start, end)
    }
}
impl TextPort for NativeText {
    fn observe(&mut self) -> Result<Observation, DesktopError> {
        let range = self.control().selection();
        let first_line = self.control().first_line()?;
        let first = self.control().line_index(first_line)?;
        let at_bottom = super::engine::find(self.hwnd)?.at_bottom()?;
        if range.cpMin < 0 || range.cpMax < range.cpMin || first < 0 {
            return Err(DesktopError::message(
                "console_selection_observe",
                "invalid native text positions",
            ));
        }
        Ok(Observation { start: range.cpMin, end: range.cpMax, first, at_bottom })
    }
    fn write(
        &mut self,
        document: &DocumentIndex,
        previous: Option<&DocumentIndex>,
    ) -> Result<(), DesktopError> {
        let retained = document.retained(previous);
        let mut copied = Vec::new();
        let count = document.units as usize + 1;
        copied
            .try_reserve_exact(count)
            .map_err(|e| DesktopError::new("console_text_readback_allocate", e))?;
        if copied.capacity() != count {
            return Err(DesktopError::message(
                "console_text_readback_allocate",
                "native readback capacity differs",
            ));
        }
        copied.resize(count, 0u16);
        let theme = if self.dark { Theme::Dark } else { Theme::Light };
        let foreground =
            rgb(console_color(Default::default(), theme, false).ok_or_else(formatting)?);
        let mut default = CHARFORMAT2W::default();
        default.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
        default.Base.dwMask = CFM_FACE | CFM_SIZE | CFM_COLOR | CFM_BACKCOLOR | CFM_BOLD;
        default.Base.yHeight = 180;
        default.Base.crTextColor = COLORREF(foreground);
        default.Base.dwEffects = CFE_AUTOBACKCOLOR;
        default.crBackColor = COLORREF(0);
        for (slot, unit) in default.Base.szFaceName.iter_mut().zip("Courier New".encode_utf16()) {
            *slot = unit;
        }
        // Native insertion owns script-specific font binding. Establish its defaults before
        // streaming, then preserve those font faces when normalizing source size and colors.
        if retained.units == 0 {
            self.control().format(super::messages::FormatScope::Default, &default)?;
        }
        if retained.units > 0 {
            if retained.trim > 0 {
                self.selection(0, retained.trim as i32)?;
                self.control().stream(&mut TextStream::slice(&[]), true)?;
            }
            let remaining = previous.ok_or_else(formatting)?.units - retained.trim;
            self.selection(retained.units as i32, remaining as i32)?;
            self.control().stream(
                &mut TextStream::slice(&document.text()[retained.units as usize..]),
                true,
            )?;
        } else {
            self.control().stream(&mut TextStream::new(document), false)?;
        }
        if self.control().length() != document.units as isize {
            return Err(DesktopError::message(
                "console_text_stream",
                "native document extent differs from exact bounded source",
            ));
        }
        if self.control().read(&mut copied)? != document.units as usize
            || copied[..document.units as usize] != *document.text()
        {
            return Err(DesktopError::message(
                "console_text_readback",
                "native text differs from unchanged source",
            ));
        }
        drop(copied);
        default.Base.dwMask &= !CFM_FACE;
        self.selection(retained.units as i32, document.units as i32)?;
        self.control().format(super::messages::FormatScope::Selection, &default)?;
        for extent in document.styles().iter().filter(|extent| extent.end > retained.units as i32) {
            let mut format = CHARFORMAT2W::default();
            format.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
            format.Base.dwMask = CFM_COLOR | CFM_BACKCOLOR | CFM_BOLD;
            format.Base.dwEffects = if extent.style.bold { CFE_BOLD } else { Default::default() };
            format.Base.crTextColor =
                COLORREF(rgb(
                    console_color(extent.style.foreground, theme, false).ok_or_else(formatting)?
                ));
            if let Some(color) = console_color(extent.style.background, theme, true) {
                format.crBackColor = COLORREF(rgb(color));
            } else {
                format.Base.dwEffects |= CFE_AUTOBACKCOLOR;
            }
            self.selection(extent.start.max(retained.units as i32), extent.end)?;
            self.control().format(super::messages::FormatScope::Selection, &format)?;
        }
        Ok(())
    }
    fn restore(
        &mut self,
        start: i32,
        end: i32,
        first: i32,
        follow: bool,
    ) -> Result<(), DesktopError> {
        self.selection(start, end)?;
        if follow {
            self.control().scroll_bottom()?;
        } else {
            let wanted = self.control().line_from_character(first)?;
            let current = self.control().first_line()?;
            if wanted < 0 || current < 0 {
                return Err(DesktopError::message(
                    "console_scroll_restore",
                    "invalid native display line",
                ));
            }
            self.control().scroll_lines(wanted - current)?;
        }
        Ok(())
    }
}
fn rgb([r, g, b, _]: [u8; 4]) -> u32 {
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16)
}
fn formatting() -> DesktopError {
    DesktopError::message("console_text_format", "native console formatting rejected")
}

#[cfg(test)]
#[path = "text/tests.rs"]
mod tests;
