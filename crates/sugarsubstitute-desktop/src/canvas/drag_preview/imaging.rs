// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode and scale drag images with Windows Imaging Component.
//!
//! WIC decodes every format the shell can show. Scaling runs on premultiplied pixels, as Qt's
//! smooth transformation did, so transparent pixels never bleed colour into their neighbours:
//! Fant area averaging shrinks and linear interpolation enlarges, matching Qt's smooth filter in
//! each direction. Results are straight-alpha RGBA, top row first.

use super::{DesktopError, worker_size};
use windows::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        Graphics::Imaging::{
            CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPRGBA, GUID_WICPixelFormat32bppRGBA,
            IWICBitmapSource, IWICImagingFactory, IWICPalette, WICBitmapDitherTypeNone,
            WICBitmapInterpolationModeFant, WICBitmapInterpolationModeLinear,
            WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
        },
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
    },
    core::{GUID, Interface},
};

/// Largest decoded edge accepted, matching the canvas's largest native viewport edge bound.
const MAX_EDGE: u32 = 65_535;

/// One decoded image: the encoded image's own size and its kept straight RGBA pixels.
pub(crate) struct Decoded {
    pub(crate) source: [u32; 2],
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

/// COM membership for the calling thread: a worker joins the multithreaded apartment for the
/// call; a thread that is already an STA (the UI thread) keeps its apartment, which WIC serves.
struct Apartment(bool);

impl Apartment {
    fn join() -> Result<Self, DesktopError> {
        // SAFETY: Paired with CoUninitialize in Drop only when this call initialized COM.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        result.ok().map_err(|cause| DesktopError::new("drag_preview_apartment", cause))?;
        Ok(Self(true))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: Balances this thread's successful CoInitializeEx; every COM object created
            // under it was released before this guard drops.
            unsafe { CoUninitialize() };
        }
    }
}

fn factory() -> Result<IWICImagingFactory, DesktopError> {
    // SAFETY: Creates an in-process WIC factory for the calling thread's apartment.
    unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
        .map_err(|cause| DesktopError::new("drag_preview_factory", cause))
}

/// Decode `bytes`, charging the full decoded frame through `admit` (given its pixel count)
/// while decoding, and keep it shrunk to fit `bound`.
pub(crate) fn decode<Admission>(
    bytes: &[u8],
    bound: [u32; 2],
    admit: impl FnOnce(u64) -> Result<Admission, DesktopError>,
) -> Result<Decoded, DesktopError> {
    let _apartment = Apartment::join()?;
    let factory = factory()?;
    let failed = |cause| DesktopError::new("drag_preview_decode", cause);
    // SAFETY: Every WIC object is used synchronously on this thread and released before return;
    // the stream borrows `bytes`, which outlives the decoder and every source pulling from it.
    let (source, frame) = unsafe {
        let stream = factory.CreateStream().map_err(failed)?;
        stream.InitializeFromMemory(bytes).map_err(failed)?;
        let decoder = factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .map_err(failed)?;
        let frame = decoder.GetFrame(0).map_err(failed)?;
        let mut size = [0_u32; 2];
        frame.GetSize(&raw mut size[0], &raw mut size[1]).map_err(failed)?;
        (size, frame)
    };
    if source.iter().any(|edge| *edge == 0 || *edge > MAX_EDGE) {
        return Err(DesktopError::message("drag_preview_decode", "Output size is unsupported"));
    }
    let _decoding = admit(u64::from(source[0]) * u64::from(source[1]))?;
    let target = worker_size(source, bound);
    let frame: IWICBitmapSource = frame.cast().map_err(failed)?;
    let rgba = scaled(&factory, &frame, source, target)?;
    Ok(Decoded { source, width: target[0], height: target[1], rgba })
}

/// Scale straight RGBA pixels of `size` to `target`.
pub(crate) fn rescale(
    rgba: &[u8],
    size: [u32; 2],
    target: [u32; 2],
) -> Result<Vec<u8>, DesktopError> {
    let _apartment = Apartment::join()?;
    let factory = factory()?;
    let failed = |cause| DesktopError::new("drag_preview_rescale", cause);
    // SAFETY: WIC copies the borrowed pixels into its own bitmap before this call returns.
    let image: IWICBitmapSource = unsafe {
        factory
            .CreateBitmapFromMemory(
                size[0],
                size[1],
                &GUID_WICPixelFormat32bppRGBA,
                size[0] * 4,
                rgba,
            )
            .map_err(failed)?
            .cast()
            .map_err(failed)?
    };
    scaled(&factory, &image, size, target)
}

/// Pull `source` through premultiplication, the direction's smooth filter and straight-alpha
/// conversion into a new RGBA buffer of `target` size.
fn scaled(
    factory: &IWICImagingFactory,
    source: &IWICBitmapSource,
    size: [u32; 2],
    target: [u32; 2],
) -> Result<Vec<u8>, DesktopError> {
    let failed = |cause| DesktopError::new("drag_preview_scale", cause);
    let length = (target[0] as usize)
        .checked_mul(target[1] as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| DesktopError::message("drag_preview_scale", "Preview size overflows"))?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(length)
        .map_err(|cause| DesktopError::new("drag_preview_pixels", cause))?;
    rgba.resize(length, 0);
    // SAFETY: Every WIC object is used synchronously on this thread; `rgba` holds exactly one
    // tightly packed 32-bit row per target row.
    unsafe {
        let premultiplied = convert(factory, source, &GUID_WICPixelFormat32bppPRGBA)?;
        let filtered = if size == target {
            premultiplied
        } else {
            let scaler = factory.CreateBitmapScaler().map_err(failed)?;
            let enlarging = target[0] > size[0] || target[1] > size[1];
            let mode = if enlarging {
                WICBitmapInterpolationModeLinear
            } else {
                WICBitmapInterpolationModeFant
            };
            scaler.Initialize(&premultiplied, target[0], target[1], mode).map_err(failed)?;
            scaler.cast().map_err(failed)?
        };
        let straight = convert(factory, &filtered, &GUID_WICPixelFormat32bppRGBA)?;
        straight.CopyPixels(std::ptr::null(), target[0] * 4, &mut rgba).map_err(failed)?;
    }
    Ok(rgba)
}

/// Convert `source` to `format`.
///
/// # Safety
/// Call on the thread that created `factory` and `source`.
unsafe fn convert(
    factory: &IWICImagingFactory,
    source: &IWICBitmapSource,
    format: &GUID,
) -> Result<IWICBitmapSource, DesktopError> {
    let failed = |cause| DesktopError::new("drag_preview_convert", cause);
    // SAFETY: Synchronous WIC calls on the caller's thread, per this function's contract.
    unsafe {
        let converter = factory.CreateFormatConverter().map_err(failed)?;
        converter
            .Initialize(
                source,
                format,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(failed)?;
        converter.cast().map_err(failed)
    }
}
