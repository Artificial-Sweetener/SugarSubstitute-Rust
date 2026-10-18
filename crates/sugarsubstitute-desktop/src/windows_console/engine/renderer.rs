// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Composite the sole RichEdit engine's native alpha over the exact admitted splash pixels.
use super::Engine;
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
        Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        Gdi::{BitBlt, HDC, SRCCOPY},
        Imaging::*,
    },
    System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
};

/// Native bitmap allocation and its target release before returning their original frame credit.
struct Glyphs {
    target: ID2D1RenderTarget,
    bitmap: IWICBitmap,
    _claim: MemoryClaim,
}
impl Glyphs {
    fn new(frame: &PreparedFrame) -> Result<Self, DesktopError> {
        let claim = frame
            .claim_storage(frame.shape().byte_len() as u64)
            .map_err(|cause| DesktopError::new("console_glyph_claim", cause))?;
        // SAFETY: The composing desktop owns an initialized STA; these are bounded OS resources.
        let (target, bitmap) = unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
                .map_err(|cause| DesktopError::new("console_d2d_factory", cause))?;
            let imaging: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                    .map_err(|cause| DesktopError::new("console_wic_factory", cause))?;
            let bitmap = imaging
                .CreateBitmap(
                    frame.width(),
                    frame.height(),
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapCacheOnLoad,
                )
                .map_err(|cause| DesktopError::new("console_wic_bitmap", cause))?;
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let target = factory
                .CreateWicBitmapRenderTarget(&bitmap, &properties)
                .map_err(|cause| DesktopError::new("console_wic_target", cause))?;
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            (target, bitmap)
        };
        Ok(Self { target, bitmap, _claim: claim })
    }
}

/// Two independently admitted native full-frame copies; the shared source pixels are never copied.
pub(super) struct Renderer {
    glyphs: Option<Glyphs>,
    output: Option<Dib>,
    source: PreparedFrame,
    document: Rc<DocumentIndex>,
    bounds: RECT,
    dark: bool,
    _paint_claim: MemoryClaim,
}
impl Renderer {
    /// Claim semantic metadata before any token allocation and both native pixel copies beforehand.
    pub(super) fn new(
        frame: &PreparedFrame,
        document: Rc<DocumentIndex>,
        bounds: RECT,
        dark: bool,
    ) -> Result<Self, DesktopError> {
        if super::super::render_proxy::PAINT_STORAGE_BYTES > NATIVE_CONSOLE_PAINT_BYTES {
            return Err(rejected("console_paint_funding"));
        }
        validate(frame, bounds)?;
        let claim = frame
            .claim_storage(NATIVE_CONSOLE_PAINT_BYTES)
            .map_err(|cause| DesktopError::new("console_paint_claim", cause))?;
        let mut owner = Self {
            glyphs: None,
            output: None,
            source: frame.clone(),
            document,
            bounds,
            dark,
            _paint_claim: claim,
        };
        owner.glyphs = Some(Glyphs::new(frame)?);
        Dib::for_frame(&mut owner.output, frame)?;
        Ok(owner)
    }
    /// Update the original immutable source only after geometry and replacement storage validate.
    pub(super) fn prepare(
        &mut self,
        frame: &PreparedFrame,
        document: Rc<DocumentIndex>,
        bounds: RECT,
        dark: bool,
    ) -> Result<(), DesktopError> {
        validate(frame, bounds)?;
        if self.source.shape() != frame.shape() {
            let replacement = Glyphs::new(frame)?;
            Dib::for_frame(&mut self.output, frame)?;
            self.glyphs = Some(replacement);
        }
        self.source = frame.clone();
        self.document = document;
        self.bounds = bounds;
        self.dark = dark;
        Ok(())
    }
    /// Publish only after native drawing and original-format/input restoration both succeeded.
    pub(super) fn paint(&mut self, engine: &Engine, dc: HDC) -> Result<(), DesktopError> {
        let glyphs = self.glyphs.as_ref().ok_or_else(|| rejected("console_glyphs_retired"))?;
        // SAFETY: The affine owner exclusively holds the real software target and draw transaction.
        unsafe {
            glyphs.target.BeginDraw();
            glyphs.target.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
        }
        let drawn = super::transaction::draw(engine, &self.document, self.dark, &glyphs.target)
            .and_then(|()| {
                if let Some(model) = super::input::Input::scroll_model(engine)? {
                    super::scrollbar::draw(&glyphs.target, &model)?;
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
        let output = self.output.as_mut().ok_or_else(|| rejected("console_output_retired"))?;
        // Copy directly into the separately charged DIB; there is no third readback allocation.
        // SAFETY: Both complete allocations have the same validated shape and exact byte extent.
        unsafe {
            glyphs.bitmap.CopyPixels(
                std::ptr::null(),
                self.source.stride() as u32,
                output.pixels_mut()?,
            )
        }
        .map_err(|cause| DesktopError::new("console_alpha_readback", cause))?;
        composite(&self.source, self.bounds, output.pixels_mut()?)?;
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
    #[cfg(test)]
    /// Read the actual native alpha plane into existing fixture-charged bytes without allocation.
    pub(super) fn read_glyphs(&self, bytes: &mut [u8]) -> Result<(), DesktopError> {
        if bytes.len() != self.source.shape().byte_len() {
            return Err(rejected("console_qualify_extent"));
        }
        let glyphs = self.glyphs.as_ref().ok_or_else(|| rejected("console_glyphs_retired"))?;
        // SAFETY: The fixture's independently charged DIB has exactly the bitmap's declared extent.
        unsafe { glyphs.bitmap.CopyPixels(std::ptr::null(), self.source.stride() as u32, bytes) }
            .map_err(|cause| DesktopError::new("console_qualify_readback", cause))
    }
}

fn validate(frame: &PreparedFrame, bounds: RECT) -> Result<(), DesktopError> {
    if bounds.left < 0
        || bounds.top < 0
        || bounds.right <= bounds.left
        || bounds.bottom <= bounds.top
        || bounds.right as u32 > frame.width()
        || bounds.bottom as u32 > frame.height()
    {
        return Err(rejected("console_frame_crop"));
    }
    Ok(())
}
/// Premultiplied source-over of native BGRA glyphs and the actual frame's RGBA crop.
fn composite(frame: &PreparedFrame, bounds: RECT, output: &mut [u8]) -> Result<(), DesktopError> {
    validate(frame, bounds)?;
    if output.len() != frame.shape().byte_len() {
        return Err(rejected("console_composite_extent"));
    }
    let width = (bounds.right - bounds.left) as usize;
    let height = (bounds.bottom - bounds.top) as usize;
    for y in 0..height {
        for x in 0..width {
            let target = y * frame.stride() + x * 4;
            let source =
                (y + bounds.top as usize) * frame.stride() + (x + bounds.left as usize) * 4;
            let alpha = output[target + 3];
            if output[target..target + 3].iter().any(|component| *component > alpha) {
                return Err(rejected("console_native_premultiplication"));
            }
            let inverse = 255 - u16::from(alpha);
            let background = &frame.pixels()[source..source + 4];
            for (channel, back) in
                [background[2], background[1], background[0], background[3]].into_iter().enumerate()
            {
                output[target + channel] = (u16::from(output[target + channel])
                    + (u16::from(back) * inverse + 127) / 255)
                    .min(255) as u8;
            }
        }
    }
    Ok(())
}
fn rejected(operation: &'static str) -> DesktopError {
    DesktopError::message(operation, "native console alpha surface contract rejected")
}
