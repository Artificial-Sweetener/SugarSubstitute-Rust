// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Composite the sole RichEdit engine's native alpha over the exact admitted splash pixels.
use super::Engine;
use crate::windows_alpha::{AlphaSurface, composite, viewport};
use crate::{DesktopError, console::index::DocumentIndex, native_storage::Dib};
use std::rc::Rc;
use sugarsubstitute_contracts::{
    execution::MemoryClaim,
    presentation::{NATIVE_CONSOLE_PAINT_BYTES, PreparedFrame},
};
use windows::Win32::{
    Foundation::RECT,
    Graphics::{
        Direct2D::{Common::*, *},
        Gdi::{BitBlt, HDC, SRCCOPY},
    },
};

/// Field copies cover their viewport. Animated transcripts retain their admitted frame capacity
/// so console disclosure does not repeatedly allocate native images while its height changes.
pub(super) struct Renderer {
    glyphs: Option<AlphaSurface>,
    output: Option<Dib>,
    source: PreparedFrame,
    transcript: Option<Rc<DocumentIndex>>,
    bounds: RECT,
    dark: bool,
    #[cfg(test)]
    pub(super) glyph_draws: usize,
    _paint_claim: Option<MemoryClaim>,
}
impl Renderer {
    /// Claim semantic metadata before any token allocation and both native pixel copies beforehand.
    pub(super) fn new(
        frame: &PreparedFrame,
        transcript: Option<Rc<DocumentIndex>>,
        bounds: RECT,
        dark: bool,
    ) -> Result<Self, DesktopError> {
        if crate::windows_controls::console::render_proxy::PAINT_STORAGE_BYTES
            > NATIVE_CONSOLE_PAINT_BYTES
        {
            return Err(rejected("console_paint_funding"));
        }
        let crop = viewport(frame, bounds)?;
        let shape = if transcript.is_some() { frame.shape() } else { crop };
        let claim = transcript
            .as_ref()
            .map(|_| frame.claim_storage(NATIVE_CONSOLE_PAINT_BYTES))
            .transpose()
            .map_err(|cause| DesktopError::new("console_paint_claim", cause))?;
        let mut owner = Self {
            glyphs: None,
            output: None,
            source: frame.clone(),
            transcript,
            bounds,
            dark,
            #[cfg(test)]
            glyph_draws: 0,
            _paint_claim: claim,
        };
        owner.glyphs = Some(AlphaSurface::new(frame, shape)?);
        Dib::for_shape(&mut owner.output, frame, shape)?;
        Ok(owner)
    }
    /// Update the original immutable source only after geometry and replacement storage validate.
    pub(super) fn prepare(
        &mut self,
        frame: &PreparedFrame,
        transcript: Option<Rc<DocumentIndex>>,
        bounds: RECT,
        dark: bool,
    ) -> Result<bool, DesktopError> {
        let crop = viewport(frame, bounds)?;
        let shape = if transcript.is_some() { frame.shape() } else { crop };
        let changed = self.source.shape() != frame.shape()
            || !match (&self.transcript, &transcript) {
                (Some(old), Some(new)) => Rc::ptr_eq(old, new),
                (None, None) => true,
                _ => false,
            }
            || self.bounds != bounds
            || self.dark != dark;
        if self.glyphs.as_ref().is_none_or(|glyphs| glyphs.shape != shape) {
            let replacement = AlphaSurface::new(frame, shape)?;
            Dib::for_shape(&mut self.output, frame, shape)?;
            self.glyphs = Some(replacement);
        }
        self.source = frame.clone();
        self.transcript = transcript;
        self.bounds = bounds;
        self.dark = dark;
        Ok(changed)
    }
    /// Publish only after native drawing and original-format/input restoration both succeeded.
    pub(super) fn paint(&mut self, engine: &Engine, dc: HDC) -> Result<(), DesktopError> {
        if engine.state.dirty.get() {
            self.rasterize(engine)?;
        }
        let glyphs = self.glyphs.as_ref().ok_or_else(|| rejected("console_glyphs_retired"))?;
        let output = self.output.as_mut().ok_or_else(|| rejected("console_output_retired"))?;
        // Copy directly into the separately charged DIB; the alpha plane remains reusable.
        // SAFETY: Both allocations have the same validated shape and exact byte extent.
        unsafe {
            glyphs.bitmap.CopyPixels(
                std::ptr::null(),
                glyphs.shape.stride() as u32,
                output.pixels_mut()?,
            )
        }
        .map_err(|cause| DesktopError::new("console_alpha_readback", cause))?;
        composite(&self.source, self.bounds, glyphs.shape, output.pixels_mut()?)?;
        // SAFETY: Only the current child's bounds are copied from the retained premultiplied DIB.
        unsafe {
            BitBlt(
                dc,
                0,
                0,
                self.bounds.right - self.bounds.left,
                self.bounds.bottom - self.bounds.top,
                Some(output.dc()),
                0,
                0,
                SRCCOPY,
            )
        }
        .map_err(|cause| DesktopError::new("console_alpha_present", cause))?;
        engine.state.check()
    }
    /// Rebuild native glyphs only after text, selection, viewport, palette or layout changes.
    fn rasterize(&mut self, engine: &Engine) -> Result<(), DesktopError> {
        let glyphs = self.glyphs.as_ref().ok_or_else(|| rejected("console_glyphs_retired"))?;
        // SAFETY: The engine retains this affine native child throughout the paint.
        let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(engine.state.hwnd) };
        if dpi == 0 {
            return Err(rejected("console_glyph_dpi"));
        }
        // SAFETY: The affine owner exclusively holds the real software target and draw transaction.
        unsafe {
            // RichEdit maps its physical layout through the native window's DPI.
            glyphs.target.SetDpi(dpi as f32, dpi as f32);
            glyphs.target.BeginDraw();
            glyphs.target.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
        }
        let drawn = match self.transcript.as_ref() {
            Some(transcript) => crate::windows_controls::console::paint::draw(
                engine,
                transcript,
                self.dark,
                &glyphs.target,
            ),
            None => draw_field(engine, &glyphs.target),
        };
        #[cfg(test)]
        {
            self.glyph_draws += 1;
        }
        // SAFETY: Custom scrollbar geometry is already expressed in physical bitmap pixels.
        unsafe { glyphs.target.SetDpi(96.0, 96.0) };
        let drawn = drawn.and_then(|()| {
            if let Some(model) = super::input::Input::scroll_model(engine)? {
                sugarsubstitute_ui::windows::scrollbar::draw(&glyphs.target, &model)?;
            }
            Ok(())
        });
        // EndDraw must be observed even if native drawing rejected a semantic paint.
        // SAFETY: This exactly matches the retained target's BeginDraw above.
        let ended = unsafe { glyphs.target.EndDraw(None, None) }
            .map_err(|cause| DesktopError::new("console_alpha_end_draw", cause));
        match (drawn, ended) {
            (Err(cause), Err(ending)) => {
                tracing::error!(operation="console_alpha_end_draw",error=%ending);
                return Err(cause);
            }
            (Err(cause), _) | (_, Err(cause)) => return Err(cause),
            _ => {}
        }
        engine.state.dirty.set(false);
        engine.state.check()
    }
    /// Retire the selected native DIB before releasing its charge, then the WIC target and bitmap.
    pub(super) fn close(&mut self) -> Result<(), DesktopError> {
        if let Some(output) = self.output.as_mut() {
            output.close()?;
        }
        self.output.take();
        self.glyphs.take();
        Ok(())
    }
    #[cfg(any(test, feature = "boundary-fixtures"))]
    /// Exercise the original native painter and borrow its already admitted BGRA output.
    pub(super) fn capture(
        &mut self,
        engine: &Engine,
        visit: &mut dyn FnMut(RECT, usize, &[u8]),
    ) -> Result<(), DesktopError> {
        let dc = self.output.as_ref().ok_or_else(|| rejected("console_capture_retired"))?.dc();
        self.paint(engine, dc)?;
        let stride =
            self.glyphs.as_ref().ok_or_else(|| rejected("console_capture_retired"))?.shape.stride();
        let output = self.output.as_mut().ok_or_else(|| rejected("console_capture_retired"))?;
        visit(self.bounds, stride, output.pixels_mut()?);
        Ok(())
    }
    #[cfg(test)]
    /// Read the actual native alpha plane into existing fixture-charged bytes without allocation.
    pub(super) fn read_glyphs(&self, bytes: &mut [u8]) -> Result<(), DesktopError> {
        if bytes.len() != self.source.shape().byte_len() {
            return Err(rejected("console_qualify_extent"));
        }
        let glyphs = self.glyphs.as_ref().ok_or_else(|| rejected("console_glyphs_retired"))?;
        bytes.fill(0);
        // SAFETY: The fixture output covers the crop with the original full-frame row stride.
        unsafe { glyphs.bitmap.CopyPixels(std::ptr::null(), self.source.stride() as u32, bytes) }
            .map_err(|cause| DesktopError::new("console_qualify_readback", cause))
    }
}

fn rejected(operation: &'static str) -> DesktopError {
    DesktopError::message(operation, "native console alpha surface contract rejected")
}

/// Paint editable text directly; the native document retains its original format and selection.
fn draw_field(engine: &Engine, target: &ID2D1RenderTarget) -> Result<(), DesktopError> {
    let mut update = engine.state.client_bounds();
    let mut logical = windows::Win32::Foundation::RECTL {
        left: update.left,
        top: update.top,
        right: update.right,
        bottom: update.bottom,
    };
    // SAFETY: The affine native document and transparent target remain retained through this draw.
    unsafe {
        engine.services()?.TxDrawD2D(
            target,
            &mut logical,
            &mut update,
            windows::Win32::UI::Controls::RichEdit::TXTVIEW_ACTIVE.0,
        )
    }
    .map_err(|cause| DesktopError::new("field_glyph_draw", cause))
}
