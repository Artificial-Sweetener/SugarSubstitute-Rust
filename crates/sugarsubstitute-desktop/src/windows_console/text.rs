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
use windows::Win32::{
    Foundation::{COLORREF, ERROR_NO_SCROLLBARS, HWND},
    Graphics::Gdi::{COLOR_WINDOW, COLOR_WINDOWTEXT, GetSysColor},
    UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        Controls::RichEdit::*,
        WindowsAndMessaging::{
            GetScrollInfo, SB_VERT, SCROLLINFO, SIF_ALL, SPI_GETHIGHCONTRAST,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
        },
    },
};
/// Publish only a fully copied and formatted retained document.
pub(super) fn publish(
    hwnd: HWND,
    documents: &mut update::Documents,
    dark: bool,
) -> Result<(), DesktopError> {
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
        let mut scroll = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_ALL,
            ..Default::default()
        };
        // SAFETY: Query only the retained peer and documented writable buffer.
        let at_bottom = match unsafe { GetScrollInfo(self.hwnd, SB_VERT, &raw mut scroll) } {
            Ok(()) => {
                scroll.nMax == scroll.nMin
                    || i64::from(scroll.nPos) + i64::from(scroll.nPage)
                        >= i64::from(scroll.nMax) + 1
            }
            // Empty or short RichEdit documents have no scroll bar. Selection and
            // first-line positions still come from this same native control.
            Err(cause)
                if cause.code() == windows::core::HRESULT::from_win32(ERROR_NO_SCROLLBARS.0) =>
            {
                true
            }
            Err(cause) => return Err(DesktopError::new("console_scroll_observe", cause)),
        };
        if range.cpMin < 0 || range.cpMax < range.cpMin || first < 0 {
            return Err(DesktopError::message(
                "console_selection_observe",
                "invalid native text positions",
            ));
        }
        Ok(Observation { start: range.cpMin, end: range.cpMax, first, at_bottom })
    }
    fn write(&mut self, document: &DocumentIndex) -> Result<(), DesktopError> {
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
        let mut reader = TextStream::new(document);
        self.control().stream(&mut reader)?;
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
        let mut high =
            HIGHCONTRASTW { cbSize: size_of::<HIGHCONTRASTW>() as u32, ..Default::default() };
        // SAFETY: Read the system's current accessibility palette into its documented structure.
        unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                high.cbSize,
                Some((&raw mut high).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
            )
        }
        .map_err(|e| DesktopError::new("console_high_contrast", e))?;
        let contrast = high.dwFlags.contains(HCF_HIGHCONTRASTON);
        let theme = if self.dark { Theme::Dark } else { Theme::Light };
        let background = if contrast {
            // SAFETY: GetSysColor returns system palette facts without retaining native resources.
            unsafe { GetSysColor(COLOR_WINDOW) }
        } else {
            rgb(if self.dark { [25; 3] } else { [255; 3] })
        };
        self.control().background(background);
        let foreground = if contrast {
            // SAFETY: Passive system palette query; no borrowed resource escapes.
            unsafe { GetSysColor(COLOR_WINDOWTEXT) }
        } else {
            rgb(console_color(Default::default(), theme, false).ok_or_else(|| {
                DesktopError::message("console_palette", "default foreground absent")
            })?)
        };
        let mut default = CHARFORMAT2W::default();
        default.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
        default.Base.dwMask = CFM_FACE | CFM_SIZE | CFM_COLOR | CFM_BACKCOLOR | CFM_BOLD;
        default.Base.yHeight = 180;
        default.Base.crTextColor = COLORREF(foreground);
        default.crBackColor = COLORREF(background);
        for (slot, unit) in default.Base.szFaceName.iter_mut().zip("Consolas".encode_utf16()) {
            *slot = unit;
        }
        self.control().format(false, &default)?;
        self.selection(0, document.units as i32)?;
        self.control().format(true, &default)?;
        for extent in document.styles() {
            let mut format = CHARFORMAT2W::default();
            format.Base.cbSize = size_of::<CHARFORMAT2W>() as u32;
            format.Base.dwMask = CFM_COLOR | CFM_BACKCOLOR | CFM_BOLD;
            format.Base.dwEffects = if extent.style.bold { CFE_BOLD } else { Default::default() };
            format.Base.crTextColor = COLORREF(if contrast {
                foreground
            } else {
                rgb(console_color(extent.style.foreground, theme, false).ok_or_else(formatting)?)
            });
            format.crBackColor = COLORREF(if contrast {
                background
            } else {
                console_color(extent.style.background, theme, true).map_or(background, rgb)
            });
            self.selection(extent.start, extent.end)?;
            self.control().format(true, &format)?;
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
            // Scrolling the caret does not give this control focus or retain a fake selection.
            self.selection(-1, -1)?;
            self.control().scroll_caret();
            self.selection(start, end)?;
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
fn rgb([r, g, b]: [u8; 3]) -> u32 {
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16)
}
fn formatting() -> DesktopError {
    DesktopError::message("console_text_format", "native console formatting rejected")
}

#[cfg(test)]
#[path = "text/tests.rs"]
mod tests;
