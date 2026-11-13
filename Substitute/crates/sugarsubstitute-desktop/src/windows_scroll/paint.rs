// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Render the shared scrollbar into a narrow, frame-funded native alpha overlay.
use super::*;
use crate::{
    native_storage::Dib,
    windows_alpha::{AlphaSurface, composite, viewport},
};
use sugarsubstitute_contracts::presentation::FrameShape;
use sugarsubstitute_ui::windows::scrollbar;
use windows::Win32::Graphics::{
    Direct2D::Common::D2D1_COLOR_F,
    Gdi::{BitBlt, GetDC, HDC, ReleaseDC, SRCCOPY},
};

pub(super) struct Paint {
    alpha: AlphaSurface,
    output: Option<Dib>,
    source: PreparedFrame,
    bounds: RECT,
    dc: Option<(HWND, HDC)>,
}
impl Paint {
    /// Charge both native copies to the original accepted frame before allocating either.
    pub(super) fn new(frame: &PreparedFrame, model: &Model) -> Result<Self, DesktopError> {
        let (bounds, shape) = crop(frame, model)?;
        let mut owner = Self {
            alpha: AlphaSurface::new(frame, shape)?,
            output: None,
            source: frame.clone(),
            bounds,
            dc: None,
        };
        Dib::for_shape(&mut owner.output, frame, shape)?;
        Ok(owner)
    }
    /// Replace shape-dependent native storage before accepting a changed frame or crop.
    pub(super) fn prepare(
        &mut self,
        frame: &PreparedFrame,
        model: &Model,
    ) -> Result<(), DesktopError> {
        self.release_dc()?;
        let (bounds, shape) = crop(frame, model)?;
        if shape != self.alpha.shape {
            let replacement = AlphaSurface::new(frame, shape)?;
            Dib::for_shape(&mut self.output, frame, shape)?;
            self.alpha = replacement;
        }
        self.alpha.fund(frame)?;
        Dib::for_shape(&mut self.output, frame, shape)?;
        self.bounds = bounds;
        self.source = frame.clone();
        Ok(())
    }
    /// Draw through the same shared Fluent painter as text scrollbars, then composite exact source pixels.
    pub(super) fn draw(
        &mut self,
        hwnd: HWND,
        mut model: Model,
        dirty: bool,
    ) -> Result<(), DesktopError> {
        self.release_dc()?;
        let output = self
            .output
            .as_mut()
            .ok_or_else(|| DesktopError::message("native_scroll_paint", "Native output retired"))?;
        if dirty {
            let shape = self.alpha.shape;
            model.bounds = RECT {
                left: 0,
                top: 0,
                right: shape.width() as i32,
                bottom: shape.height() as i32,
            };
            // SAFETY: This owner exclusively holds the admitted, affine native draw target.
            unsafe {
                self.alpha.target.BeginDraw();
                self.alpha.target.Clear(Some(&D2D1_COLOR_F::default()));
            }
            let painted = scrollbar::draw(&self.alpha.target, &model).map_err(DesktopError::from);
            // SAFETY: Observe EndDraw even when shared widget painting failed.
            let ended = unsafe { self.alpha.target.EndDraw(None, None) }
                .map_err(|e| DesktopError::new("native_scroll_draw_end", e));
            if let Err(cause) = painted {
                if let Err(end) = ended {
                    tracing::error!(operation="native_scroll_draw_end", cause=%end);
                }
                return Err(cause);
            }
            ended?;
            // SAFETY: The admitted output has the same exact extent and stride as this alpha bitmap.
            unsafe {
                self.alpha.bitmap.CopyPixels(
                    std::ptr::null(),
                    shape.stride() as u32,
                    output.pixels_mut()?,
                )
            }
            .map_err(|e| DesktopError::new("native_scroll_alpha_copy", e))?;
            composite(&self.source, self.bounds, shape, output.pixels_mut()?)?;
        }
        // SAFETY: Acquire the original parent's DC only for this synchronous native publication.
        let dc = unsafe { GetDC(Some(hwnd)) };
        if dc.is_invalid() {
            return Err(DesktopError::message("native_scroll_dc", "Window DC unavailable"));
        }
        self.dc = Some((hwnd, dc));
        // SAFETY: Copy only the validated gutter from retained native storage into the original parent.
        let copied = unsafe {
            BitBlt(
                dc,
                self.bounds.left,
                self.bounds.top,
                self.bounds.right - self.bounds.left,
                self.bounds.bottom - self.bounds.top,
                Some(output.dc()),
                0,
                0,
                SRCCOPY,
            )
        };
        let copied = copied.map_err(|e| DesktopError::new("native_scroll_present", e));
        let retired = self.release_dc();
        match (copied, retired) {
            (Err(cause), Err(retirement)) => {
                tracing::error!(operation="native_scroll_dc_release", cause=%retirement);
                Err(cause)
            }
            (Err(cause), _) | (_, Err(cause)) => Err(cause),
            _ => Ok(()),
        }
    }
    /// Keep a failed release retained for checked shutdown; never forget a successfully acquired DC.
    fn release_dc(&mut self) -> Result<(), DesktopError> {
        if let Some((hwnd, dc)) = self.dc {
            // SAFETY: Return only the exact DC acquired and retained by this original paint owner.
            if unsafe { ReleaseDC(Some(hwnd), dc) } != 1 {
                return Err(DesktopError::message(
                    "native_scroll_dc_release",
                    "Window DC retirement failed",
                ));
            }
            self.dc = None;
        }
        Ok(())
    }
    #[cfg(any(test, feature = "boundary-fixtures"))]
    /// Borrow the exact last painted page scrollbar without issuing a second draw.
    pub(super) fn capture(
        &mut self,
        observer: &mut dyn crate::pixel_capture::PixelCapture,
    ) -> Result<(), DesktopError> {
        let output = self
            .output
            .as_mut()
            .ok_or_else(|| DesktopError::message("native_scroll_capture", "Output retired"))?;
        let b = self.bounds;
        observer.overlay(
            None,
            [b.left, b.top, b.right, b.bottom],
            None,
            self.alpha.shape.stride(),
            output.pixels_mut()?,
        );
        Ok(())
    }
    /// Retire the selected DIB before its original frame can release storage credit.
    pub(super) fn close(&mut self) -> Result<(), DesktopError> {
        self.release_dc()?;
        if let Some(output) = &mut self.output {
            output.close()?;
        }
        self.output = None;
        Ok(())
    }
}
/// Keep the same gutter width used by native form composition, in exact physical crop coordinates.
fn crop(frame: &PreparedFrame, model: &Model) -> Result<(RECT, FrameShape), DesktopError> {
    let width = (f64::from(sugarsubstitute_ui::form::SCROLL_GUTTER) * model.scale).ceil() as i32;
    let bounds = RECT { left: (model.bounds.right - width).max(model.bounds.left), ..model.bounds };
    Ok((bounds, viewport(frame, bounds)?))
}
