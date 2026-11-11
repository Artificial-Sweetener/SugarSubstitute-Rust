// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read back the exact composition-surface pixels written by one test draw.

use crate::DesktopError;
use sugarsubstitute_image_ops::Extent;
use windows::{
    Win32::{
        Foundation::POINT,
        Graphics::{
            Direct2D::{
                Common::{D2D_RECT_F, D2D1_COLOR_F},
                D2D1_ANTIALIAS_MODE_ALIASED, ID2D1Bitmap1, ID2D1DeviceContext,
            },
            Direct3D11::{
                D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
                D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11DeviceContext, ID3D11Texture2D,
            },
            Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
        },
    },
    core::Interface,
};

/// One requested readback of the drawn update rectangle of a composition surface.
#[derive(Default)]
pub(in super::super) struct Capture {
    requested: bool,
    /// Opaque color to leave in the next update rectangle before the production draw, as
    /// DirectComposition may hand back a buffer still holding an earlier frame.
    stale: Option<u32>,
    pending: Option<(ID3D11Texture2D, ID3D11DeviceContext, Extent)>,
    pixels: Option<Vec<u32>>,
}

impl Capture {
    /// Capture the next drawn frame.
    pub(in super::super) fn request(&mut self) {
        self.requested = true;
    }

    /// Leave opaque `0xffRRGGBB` content in the next update rectangle before it is drawn.
    pub(in super::super) fn leave_stale(&mut self, color: u32) {
        self.stale = Some(color);
    }

    /// Fill the update rectangle with the requested stale content, if any.
    pub(super) fn prefill(&mut self, context: &ID2D1DeviceContext, rectangle: &D2D_RECT_F) {
        let Some(color) = self.stale.take() else { return };
        #[allow(clippy::cast_precision_loss)]
        let channel = |shift: u32| ((color >> shift) & 0xff) as f32 / 255.0;
        let fill = D2D1_COLOR_F { r: channel(16), g: channel(8), b: channel(0), a: 1.0 };
        // SAFETY: The test thread owns this active drawing context; the clip is popped here.
        unsafe {
            context.PushAxisAlignedClip(rectangle, D2D1_ANTIALIAS_MODE_ALIASED);
            context.Clear(Some(&fill));
            context.PopAxisAlignedClip();
        }
    }

    /// Take completed `0xAARRGGBB` words in row-major order.
    pub(in super::super) fn take(&mut self) -> Option<Vec<u32>> {
        self.pixels.take()
    }

    /// Queue a copy of the surface's drawn rectangle before the draw ends.
    pub(super) fn copy(
        &mut self,
        context: &ID2D1DeviceContext,
        offset: POINT,
        extent: Extent,
    ) -> Result<(), DesktopError> {
        if !std::mem::take(&mut self.requested) {
            return Ok(());
        }
        let fail = |cause| DesktopError::new("capture_canvas_surface", cause);
        // SAFETY: The test thread owns this active drawing context and its surface target.
        unsafe {
            context.Flush(None, None).map_err(fail)?;
            let target = context.GetTarget().map_err(fail)?;
            let bitmap: ID2D1Bitmap1 = target.cast().map_err(fail)?;
            let surface = bitmap.GetSurface().map_err(fail)?;
            let texture: ID3D11Texture2D = surface.cast().map_err(fail)?;
            let device = texture.GetDevice().map_err(fail)?;
            let immediate = device.GetImmediateContext().map_err(fail)?;
            let description = D3D11_TEXTURE2D_DESC {
                Width: extent.width(),
                Height: extent.height(),
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut staging = None;
            device.CreateTexture2D(&description, None, Some(&mut staging)).map_err(fail)?;
            let staging = staging.ok_or_else(|| {
                DesktopError::message("capture_canvas_surface", "staging texture missing")
            })?;
            let left = u32::try_from(offset.x).map_err(|_| {
                DesktopError::message("capture_canvas_surface", "negative surface offset")
            })?;
            let top = u32::try_from(offset.y).map_err(|_| {
                DesktopError::message("capture_canvas_surface", "negative surface offset")
            })?;
            let source = D3D11_BOX {
                left,
                top,
                front: 0,
                right: left + extent.width(),
                bottom: top + extent.height(),
                back: 1,
            };
            immediate.CopySubresourceRegion(&staging, 0, 0, 0, 0, &texture, 0, Some(&source));
            self.pending = Some((staging, immediate, extent));
        }
        Ok(())
    }

    /// Map the queued copy after the draw ended and the surface update was submitted.
    pub(super) fn complete(&mut self) -> Result<(), DesktopError> {
        let Some((staging, immediate, extent)) = self.pending.take() else { return Ok(()) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: The staging texture is CPU-readable and owned by this test capture.
        unsafe { immediate.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
            .map_err(|cause| DesktopError::new("map_canvas_capture", cause))?;
        let width = extent.width() as usize;
        let mut pixels = Vec::with_capacity(width * extent.height() as usize);
        for row in 0..extent.height() as usize {
            // SAFETY: Mapping covers `RowPitch * height` bytes of BGRA8 words until Unmap.
            let words = unsafe {
                std::slice::from_raw_parts(
                    mapped.pData.cast::<u8>().add(row * mapped.RowPitch as usize).cast::<u32>(),
                    width,
                )
            };
            pixels.extend_from_slice(words);
        }
        // SAFETY: This unmaps the subresource mapped above.
        unsafe { immediate.Unmap(&staging, 0) };
        self.pixels = Some(pixels);
        Ok(())
    }
}
