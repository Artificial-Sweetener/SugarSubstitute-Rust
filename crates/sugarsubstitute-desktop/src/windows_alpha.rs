// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Shared admitted Windows alpha surfaces for native text and widget overlays.
use crate::DesktopError;
use sugarsubstitute_contracts::{
    execution::MemoryClaim,
    presentation::{FrameShape, PreparedFrame},
};
use windows::Win32::{
    Foundation::RECT,
    Graphics::{
        Direct2D::{Common::*, *},
        Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        Imaging::*,
    },
    System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
};
/// Native bitmap allocation and its target release before returning their admitted storage credit.
pub(crate) struct AlphaSurface {
    pub(crate) target: ID2D1RenderTarget,
    pub(crate) bitmap: IWICBitmap,
    pub(crate) shape: FrameShape,
    _claim: MemoryClaim,
}
impl AlphaSurface {
    /// Keep reusable native pixels charged to the accepted frame without retaining obsolete work.
    pub(crate) fn fund(&mut self, frame: &PreparedFrame) -> Result<(), DesktopError> {
        frame
            .fund_storage(&mut self._claim)
            .map_err(|cause| DesktopError::new("console_glyph_funding", cause))
    }

    /// Claim the exact bitmap extent before creating the retained affine software target.
    pub(crate) fn new(frame: &PreparedFrame, shape: FrameShape) -> Result<Self, DesktopError> {
        let claim = frame
            .claim_storage(shape.byte_len() as u64)
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
                    shape.width(),
                    shape.height(),
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
        Ok(Self { target, bitmap, shape, _claim: claim })
    }
}

/// Validate the crop against its source before admitting native backing storage.
pub(crate) fn viewport(frame: &PreparedFrame, bounds: RECT) -> Result<FrameShape, DesktopError> {
    if bounds.left < 0
        || bounds.top < 0
        || bounds.right <= bounds.left
        || bounds.bottom <= bounds.top
        || bounds.right as u32 > frame.width()
        || bounds.bottom as u32 > frame.height()
    {
        return Err(rejected("console_frame_crop"));
    }
    FrameShape::new(
        (bounds.right - bounds.left) as u32,
        (bounds.bottom - bounds.top) as u32,
        frame.scale(),
    )
    .map_err(|cause| DesktopError::new("console_native_viewport", cause))
}
/// Premultiplied source-over of native BGRA glyphs and the actual frame's RGBA crop.
pub(crate) fn composite(
    frame: &PreparedFrame,
    bounds: RECT,
    shape: FrameShape,
    output: &mut [u8],
) -> Result<(), DesktopError> {
    let crop = viewport(frame, bounds)?;
    if shape.width() < crop.width() || shape.height() < crop.height() {
        return Err(rejected("console_composite_crop"));
    }
    if output.len() != shape.byte_len() {
        return Err(rejected("console_composite_extent"));
    }
    let width = (bounds.right - bounds.left) as usize;
    let height = (bounds.bottom - bounds.top) as usize;
    for y in 0..height {
        for x in 0..width {
            let target = y * shape.stride() + x * 4;
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
    DesktopError::message(operation, "native alpha surface contract rejected")
}
