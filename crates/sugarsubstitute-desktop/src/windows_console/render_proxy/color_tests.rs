// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect native color-font layers and ordinary semantic opacity in a bounded WIC target.
use super::*;
use std::mem::ManuallyDrop;
use windows::{
    Win32::{
        Graphics::{DirectWrite::*, Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM, Imaging::*},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
    },
    core::{BOOL, PCWSTR, w},
};
use windows_numerics::Vector2;

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: This test successfully initialized one matching apartment on the same thread.
        unsafe { CoUninitialize() };
    }
}

struct Run(DWRITE_GLYPH_RUN);
impl Drop for Run {
    fn drop(&mut self) {
        // SAFETY: This test owns the sole ManuallyDrop field and retires all native borrows first.
        unsafe { ManuallyDrop::drop(&mut self.0.fontFace) };
    }
}

fn font(
    factory: &IDWriteFactory,
    family: PCWSTR,
) -> Result<IDWriteFontFace, Box<dyn std::error::Error>> {
    let mut collection = None;
    // SAFETY: Owned optional COM output and system-font query remain local to this test.
    unsafe { factory.GetSystemFontCollection(&mut collection, false)? };
    let collection = collection.ok_or("system font collection absent")?;
    let mut index = 0;
    let mut exists = BOOL::default();
    // SAFETY: Font-family literal and scalar outputs remain live throughout native lookup.
    unsafe {
        collection.FindFamilyName(family, &mut index, &mut exists)?;
        if !exists.as_bool() {
            return Err("required Windows system font absent".into());
        }
        Ok(collection
            .GetFontFamily(index)?
            .GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )?
            .CreateFontFace()?)
    }
}

fn render(
    face: &IDWriteFontFace,
    codepoint: u32,
    alpha: u8,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // SAFETY: The initialized test apartment owns all bounded WIC and Direct2D resources.
    let (bitmap, target) = unsafe {
        let imaging: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let bitmap =
            imaging.CreateBitmap(64, 64, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad)?;
        let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
        let target = factory.CreateWicBitmapRenderTarget(
            &bitmap,
            &D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            },
        )?;
        target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        (bitmap, target)
    };
    let proxy =
        Proxy::new(target.clone(), vec![PaintColor { token: 1, rgba: [255, 255, 255, alpha] }])?;
    let mut glyph = 0;
    // SAFETY: One native codepoint and its glyph output remain owned throughout lookup.
    unsafe { face.GetGlyphIndices(&codepoint, 1, &mut glyph)? };
    assert_ne!(glyph, 0);
    let advance = 52.0;
    let run = Run(DWRITE_GLYPH_RUN {
        fontFace: ManuallyDrop::new(Some(face.clone())),
        fontEmSize: 48.0,
        glyphCount: 1,
        glyphIndices: &glyph,
        glyphAdvances: &advance,
        ..Default::default()
    });
    // SAFETY: The token color, target transaction and shaped test run remain owned until EndDraw.
    unsafe {
        let brush = proxy.interface().CreateSolidColorBrush(
            &D2D1_COLOR_F { r: 1.0 / 255.0, g: 0.0, b: 0.0, a: 1.0 },
            None,
        )?;
        target.BeginDraw();
        target.Clear(Some(&D2D1_COLOR_F::default()));
        proxy.interface().DrawGlyphRun(
            Vector2 { X: 4.0, Y: 52.0 },
            &run.0,
            &brush,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        target.EndDraw(None, None)?;
    }
    proxy.finish()?;
    let mut pixels = vec![0; 64 * 64 * 4];
    // SAFETY: The owned exact-sized PBGRA output is writable throughout synchronous pixel copy.
    unsafe { bitmap.CopyPixels(core::ptr::null(), 64 * 4, &mut pixels)? };
    for pixel in pixels.chunks_exact(4) {
        assert!(pixel[..3].iter().all(|channel| *channel <= pixel[3]));
    }
    Ok(pixels)
}

#[test]
fn native_palette_emoji_keeps_intrinsic_colors_while_outline_text_keeps_semantic_opacity()
-> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: This test balances its successful per-thread apartment entry with its local guard.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
    let _apartment = Apartment;
    // SAFETY: The factory is owned and used only by this test's initialized native thread.
    let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    let plain = font(&factory, w!("Courier New"))?;
    for alpha in [234, 255] {
        let pixels = render(&plain, u32::from('H'), alpha)?;
        assert_eq!(pixels.chunks_exact(4).map(|pixel| pixel[3]).max(), Some(alpha));
        assert!(pixels.chunks_exact(4).all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2]));
    }
    let emoji = font(&factory, w!("Segoe UI Emoji"))?;
    let pixels = render(&emoji, 0x1f600, 234)?;
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] != pixel[1] || pixel[1] != pixel[2]));
    assert_eq!(pixels.chunks_exact(4).map(|pixel| pixel[3]).max(), Some(255));
    Ok(())
}
