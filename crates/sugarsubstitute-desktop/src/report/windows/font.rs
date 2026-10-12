// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain at most two actual DPI fonts through partial replacement and child destruction.
use super::super::ReportSurfaceError;
use windows::Win32::{
    Graphics::Gdi::{CreateFontIndirectW, DeleteObject, HFONT, HGDIOBJ},
    UI::{
        HiDpi::SystemParametersInfoForDpi,
        WindowsAndMessaging::{NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS},
    },
};
pub(super) trait Boundary {
    fn create(&mut self, dpi: u32) -> Result<HFONT, ReportSurfaceError>;
    fn destroy(&mut self, font: HFONT) -> Result<(), ReportSurfaceError>;
}
pub(super) struct Native;
impl Boundary for Native {
    fn create(&mut self, dpi: u32) -> Result<HFONT, ReportSurfaceError> {
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        // SAFETY: Bounded writable Unicode metrics; query actual caller-specified DPI, no global settings change.
        unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                metrics.cbSize,
                Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
                0,
                dpi,
            )
        }
        .map_err(|cause| ReportSurfaceError::new("report_font_metrics", cause))?;
        // SAFETY: The returned fixed Unicode LOGFONTW initializes one immediately adopted GDI object.
        let font = unsafe { CreateFontIndirectW(&metrics.lfMessageFont) };
        if font.is_invalid() {
            return Err(ReportSurfaceError::new(
                "report_font_create",
                std::io::Error::other("GDI font creation returned null"),
            ));
        }
        Ok(font)
    }
    fn destroy(&mut self, font: HFONT) -> Result<(), ReportSurfaceError> {
        // SAFETY: This exact font is no longer selected by any retained child or DC.
        if !unsafe { DeleteObject(HGDIOBJ(font.0)) }.as_bool() {
            return Err(ReportSurfaceError::new(
                "report_font_destroy",
                std::io::Error::other("GDI font retirement returned zero"),
            ));
        }
        Ok(())
    }
}
pub(super) struct Fonts<B = Native> {
    boundary: B,
    current: Option<HFONT>,
    pending: Option<HFONT>,
    dpi: u32,
    pending_dpi: u32,
}
impl Fonts<Native> {
    pub(super) const fn new() -> Self {
        Self { boundary: Native, current: None, pending: None, dpi: 0, pending_dpi: 0 }
    }
}
impl<B: Boundary> Fonts<B> {
    pub(super) fn prepare(&mut self, dpi: u32) -> Result<Option<HFONT>, ReportSurfaceError> {
        if self.pending.is_some() || !(48..=768).contains(&dpi) {
            return Err(ReportSurfaceError::rejected("report_font_admission"));
        }
        if self.current.is_some() && dpi == self.dpi {
            return Ok(None);
        }
        let font = self.boundary.create(dpi)?;
        self.pending = Some(font);
        self.pending_dpi = dpi;
        Ok(Some(font))
    }
    pub(super) fn commit(&mut self) -> Result<(), ReportSurfaceError> {
        if self.pending.is_none() {
            return Err(ReportSurfaceError::rejected("report_font_commit"));
        }
        if let Some(current) = self.current {
            self.boundary.destroy(current)?;
            self.current = None;
        }
        self.current = self.pending.take();
        self.dpi = self.pending_dpi;
        Ok(())
    }
    /// Caller first proves every native child/DC/callback actually destroyed or detached its font.
    pub(super) fn retire_after_children(&mut self) -> Result<(), ReportSurfaceError> {
        if let Some(pending) = self.pending {
            self.boundary.destroy(pending)?;
            self.pending = None;
        }
        if let Some(current) = self.current {
            self.boundary.destroy(current)?;
            self.current = None;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
