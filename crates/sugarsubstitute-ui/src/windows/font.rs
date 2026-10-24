// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain at most two actual DPI fonts through partial replacement and child destruction.
use super::{NativeUiError, text_style::TextStyle};
use windows::Win32::{
    Graphics::Gdi::{CreateFontIndirectW, DeleteObject, HFONT, HGDIOBJ},
    UI::{
        HiDpi::SystemParametersInfoForDpi,
        WindowsAndMessaging::{NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS},
    },
};
/// Native allocation boundary; callers keep all fonts affine to their creating UI thread.
pub trait Boundary {
    /// Allocate one owned font at the requested Windows DPI.
    /// # Errors
    /// Preserves the actual metrics or GDI allocation failure.
    fn create(&mut self, dpi: u32) -> Result<HFONT, NativeUiError>;
    /// Release a font after every original child has detached it.
    /// # Errors
    /// Preserves failed deletion for a later checked cleanup attempt.
    fn destroy(&mut self, font: HFONT) -> Result<(), NativeUiError>;
}
/// Shared native font roles; editable fields and selectors use identical typography.
#[derive(Clone, Copy)]
pub enum FontRole {
    /// Current system message font, used by OS recovery/reporting surfaces.
    Message,
    /// The same Fluent font specification as editable fields.
    Form,
}
/// Actual GDI allocation boundary, retained inside the font owner.
pub struct Native {
    role: FontRole,
}
impl Boundary for Native {
    fn create(&mut self, dpi: u32) -> Result<HFONT, NativeUiError> {
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
        .map_err(|cause| NativeUiError::new("native_font_metrics", cause))?;
        if matches!(self.role, FontRole::Form) {
            let format = TextStyle::Field.font();
            metrics.lfMessageFont.lfFaceName = format.Base.szFaceName;
            metrics.lfMessageFont.lfHeight =
                -((i64::from(format.Base.yHeight) * i64::from(dpi) + 720) / 1440) as i32;
            metrics.lfMessageFont.lfWeight = 400;
            metrics.lfMessageFont.lfItalic = 0;
        }
        // SAFETY: The returned fixed Unicode LOGFONTW initializes one immediately adopted GDI object.
        let font = unsafe { CreateFontIndirectW(&metrics.lfMessageFont) };
        if font.is_invalid() {
            return Err(NativeUiError::new(
                "native_font_create",
                std::io::Error::other("GDI font creation returned null"),
            ));
        }
        Ok(font)
    }
    fn destroy(&mut self, font: HFONT) -> Result<(), NativeUiError> {
        // SAFETY: This exact font is no longer selected by any retained child or DC.
        if !unsafe { DeleteObject(HGDIOBJ(font.0)) }.as_bool() {
            return Err(NativeUiError::new(
                "native_font_destroy",
                std::io::Error::other("GDI font retirement returned zero"),
            ));
        }
        Ok(())
    }
}
/// Retain old and replacement native fonts until actual widget adoption or retirement is proven.
pub struct Fonts<B = Native> {
    boundary: B,
    current: Option<HFONT>,
    pending: Option<HFONT>,
    dpi: u32,
    pending_dpi: u32,
}
impl Fonts<Native> {
    /// Start without allocating native storage; this owner must outlive every borrowing widget.
    pub const fn new(role: FontRole) -> Self {
        Self { boundary: Native { role }, current: None, pending: None, dpi: 0, pending_dpi: 0 }
    }
}
impl<B: Boundary> Fonts<B> {
    /// Allocate at most one replacement while retaining the original font.
    /// # Errors
    /// Rejects invalid DPI or an unresolved replacement and preserves native allocation failure.
    pub fn prepare(&mut self, dpi: u32) -> Result<Option<HFONT>, NativeUiError> {
        if self.pending.is_some() || !(48..=768).contains(&dpi) {
            return Err(NativeUiError::message(
                "native_font_admission",
                "Native font admission rejected",
            ));
        }
        if self.current.is_some() && dpi == self.dpi {
            return Ok(None);
        }
        let font = self.boundary.create(dpi)?;
        self.pending = Some(font);
        self.pending_dpi = dpi;
        Ok(Some(font))
    }
    /// Commit only after every original widget has switched to the pending font.
    /// # Errors
    /// Retains both fonts when old-font retirement fails; rejects a missing replacement.
    pub fn commit(&mut self) -> Result<(), NativeUiError> {
        if self.pending.is_none() {
            return Err(NativeUiError::message(
                "native_font_commit",
                "Native font replacement absent",
            ));
        }
        if let Some(current) = self.current {
            self.boundary.destroy(current)?;
            self.current = None;
        }
        self.current = self.pending.take();
        self.dpi = self.pending_dpi;
        Ok(())
    }
    /// Borrow the replacement during a transaction, or the committed font otherwise.
    pub fn selected(&self) -> Option<HFONT> {
        self.pending.or(self.current)
    }
    /// Report actual retirement, including a replacement that never reached commit.
    pub fn retired(&self) -> bool {
        self.pending.is_none() && self.current.is_none()
    }
    /// Caller first proves every native child/DC/callback actually destroyed or detached its font.
    /// # Errors
    /// Failed deletion retains the exact pending/current handle for checked retry.
    pub fn retire_after_children(&mut self) -> Result<(), NativeUiError> {
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
