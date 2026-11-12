// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise exact semantic identity and native COM interception without a window or device DC.
use super::*;

#[test]
fn semantic_failure_projection_preserves_closed_native_details() {
    use crate::console_failure_diagnostic::ConsoleFailureCode as Code;
    for (detail, expected) in [
        (Detail::Color { bits: [0; 4], token: Some(0x123456) }, Code::UnknownPaintToken(0x123456)),
        (Detail::Color { bits: [0; 4], token: None }, Code::InvalidPaintColor),
        (Detail::GlyphFormat { glyph: 1, formats: 128 }, Code::UnsupportedGlyphFormat(128)),
        (Detail::Unsupported, Code::UnsupportedPaint),
        (Detail::Native, Code::Hresult(E_INVALIDARG.0)),
    ] {
        let cause = NativeFailure {
            cause: E_INVALIDARG.into(),
            context: Context { method: "DrawGlyphRun", detail, glyph: None },
        };
        assert_eq!(diagnostic_code(&cause), Some(expected));
    }
    assert_eq!(diagnostic_code(&std::io::Error::other("Unrelated error")), None);
}
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

fn encoded(value: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (value & 255) as f32 / 255.0,
        g: ((value >> 8) & 255) as f32 / 255.0,
        b: ((value >> 16) & 255) as f32 / 255.0,
        a: 1.0,
    }
}

#[test]
fn semantic_dictionary_rejects_ambiguity_and_unbounded_storage() {
    assert!(validate(&[]).is_err());
    assert!(validate(&[PaintColor { token: 0x100_0000, rgba: [0; 4] }]).is_err());
    assert!(
        validate(&[PaintColor { token: 9, rgba: [1; 4] }, PaintColor { token: 9, rgba: [2; 4] },])
            .is_err()
    );
    assert!(
        validate(&[PaintColor { token: 9, rgba: [1; 4] }, PaintColor { token: 8, rgba: [2; 4] },])
            .is_err()
    );
    let oversized: Vec<_> = (0..MAX_PAINT_COLORS + 1)
        .map(|token| PaintColor { token: token as u32, rgba: [0; 4] })
        .collect();
    assert!(validate(&oversized).is_err());
}

#[test]
fn every_style_foreground_and_background_plus_default_and_selection_tokens_fit() {
    let maximum: Vec<_> = (0..MAX_PAINT_COLORS)
        .map(|token| PaintColor { token: token as u32, rgba: [255, 255, 255, 234] })
        .collect();
    assert_eq!(maximum.len(), 32_772);
    assert!(validate(&maximum).is_ok());
    assert!(PAINT_STORAGE_BYTES >= (maximum.capacity() * size_of::<PaintColor>()) as u64);
}

#[test]
fn tokens_accept_every_exact_channel_and_reject_fractional_or_nonfinite_paints() {
    for byte in 0..=255 {
        let value = byte | (byte << 8) | (byte << 16);
        assert_eq!(token(encoded(value)).ok(), Some(value));
        let native = byte as f32 * (1.0 / 255.0);
        assert_eq!(
            token(D2D1_COLOR_F { r: native, g: native, b: native, a: 1.0 }).ok(),
            Some(value)
        );
    }
    for bad in [f32::NAN, f32::INFINITY, -0.001, 1.001, 0.12345] {
        assert!(token(D2D1_COLOR_F { r: bad, ..encoded(0) }).is_err());
    }
    for alpha in [0.0, 0.92, f32::NAN] {
        assert!(token(D2D1_COLOR_F { a: alpha, ..encoded(0) }).is_err());
    }
}

fn target() -> windows::core::Result<ID2D1RenderTarget> {
    // SAFETY: A single-thread factory and unbound memory-only target are owned by this test.
    let factory: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
    // SAFETY: The complete input is borrowed for creation; no HWND, HDC or desktop is used.
    unsafe {
        factory
            .CreateDCRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                ..Default::default()
            })?
            .cast()
    }
}

#[test]
fn native_proxy_retains_identity_and_first_rejection_without_leaking_real_target()
-> Result<(), Box<dyn std::error::Error>> {
    let real = target()?;
    let mut oversized = Vec::with_capacity(MAX_PAINT_COLORS + 1);
    oversized.push(PaintColor { token: 1, rgba: [255; 4] });
    assert!(Proxy::new(real.clone(), oversized).is_err());
    let proxy =
        Proxy::new(real.clone(), vec![PaintColor { token: 1, rgba: [255, 255, 255, 234] }])?;
    assert_ne!(proxy.interface().as_raw(), real.as_raw());
    assert!(proxy.interface().cast::<ID2D1DCRenderTarget>().is_err());
    let resource: ID2D1Resource = proxy.interface().cast()?;
    let rebound: ID2D1RenderTarget = resource.cast()?;
    assert_eq!(proxy.interface().as_raw(), rebound.as_raw());
    let mut parameters = core::ptr::null_mut();
    // SAFETY: The optional owned COM output is valid for the complete synchronous query.
    unsafe {
        (Interface::vtable(proxy.interface()).GetTextRenderingParams)(
            proxy.interface().as_raw(),
            &mut parameters,
        );
    }
    if !parameters.is_null() {
        // SAFETY: The nonnull native output transfers one owned rendering-parameters reference.
        drop(unsafe {
            windows::Win32::Graphics::DirectWrite::IDWriteRenderingParams::from_raw(parameters)
        });
    }
    proxy.finish()?;
    // SAFETY: This unsupported call is intercepted before it can alter the real target.
    unsafe { proxy.interface().BeginDraw() };
    assert_eq!(proxy.shared.failure.get(), E_NOTIMPL);
    assert!(proxy.finish().is_err());
    let first = proxy.shared.context.get();
    assert_eq!(first.method, "BeginDraw");
    proxy.shared.fail(E_INVALIDARG);
    assert_eq!(proxy.shared.failure.get(), E_NOTIMPL);
    assert_eq!(proxy.shared.context.get(), first);
    let diagnostic = proxy.finish().err().ok_or("missing first-failure diagnostic")?.to_string();
    assert!(diagnostic.contains("method=BeginDraw"));
    assert!(diagnostic.contains("unsupported render operation"));
    Ok(())
}

#[test]
fn native_brush_tokens_preserve_distinct_semantics_and_restore_original_brush_state()
-> Result<(), Box<dyn std::error::Error>> {
    let real = target()?;
    let proxy = Proxy::new(
        real,
        vec![
            PaintColor { token: 1, rgba: [255, 255, 255, 234] },
            PaintColor { token: 2, rgba: [255, 255, 255, 255] },
        ],
    )?;
    assert_ne!(proxy.shared.color(encoded(1))?.a, proxy.shared.color(encoded(2))?.a);
    // SAFETY: Creating a solid brush only retains its value in this owned unbound target.
    let solid = unsafe { proxy.interface().CreateSolidColorBrush(&encoded(1), None)? };
    // SAFETY: The owned brush remains live throughout each synchronous observation.
    unsafe { solid.SetOpacity(0.75) };
    let brush: ID2D1Brush = solid.cast()?;
    let observed = Cell::new(false);
    proxy.shared.paint("fixture", &(&brush).into(), || {
        // SAFETY: The paint callback retains the same owned native brush until restoration.
        let current = unsafe { solid.GetColor() };
        assert_eq!([current.r, current.g, current.b, current.a], [1.0, 1.0, 1.0, 234.0 / 255.0]);
        observed.set(true);
    });
    assert!(observed.get());
    // SAFETY: Original native brush state remains owned after the completed callback.
    unsafe {
        assert_eq!(solid.GetColor(), encoded(1));
        assert_eq!(solid.GetOpacity(), 0.75);
        solid.SetColor(&encoded(3));
    }
    observed.set(false);
    proxy.shared.paint("fixture", &(&brush).into(), || observed.set(true));
    assert!(!observed.get());
    assert_eq!(proxy.shared.failure.get(), E_INVALIDARG);
    let diagnostic = proxy.finish().err().ok_or("missing token diagnostic")?.to_string();
    assert!(diagnostic.contains("method=fixture"));
    assert!(diagnostic.contains("unknown_token=0x000003"));
    assert!(diagnostic.contains("brush_bits="));
    let rejected =
        Proxy::new(proxy.shared.target.clone(), vec![PaintColor { token: 1, rgba: [255; 4] }])?;
    let invalid = D2D1_COLOR_F { r: 0.12345, ..encoded(1) };
    // SAFETY: This owned memory-only brush preserves the exact invalid normalization for the test.
    let solid = unsafe { rejected.interface().CreateSolidColorBrush(&invalid, None)? };
    let brush: ID2D1Brush = solid.cast()?;
    let run = windows::Win32::Graphics::DirectWrite::DWRITE_GLYPH_RUN {
        fontEmSize: 13.0,
        ..Default::default()
    };
    let scope = rejected
        .shared
        .glyph_scope(&run, windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL);
    rejected.shared.paint("DrawGlyphRun", &(&brush).into(), || observed.set(true));
    drop(scope);
    assert!(rejected.shared.active_glyph.get().is_none());
    let diagnostic =
        rejected.finish().err().ok_or("missing normalized-token diagnostic")?.to_string();
    assert!(diagnostic.contains("invalid normalized opaque token"));
    assert!(diagnostic.contains("glyph_count=0"));
    assert!(diagnostic.contains("font_em=13"));
    assert!(diagnostic.contains("first_glyph=None"));
    Ok(())
}

#[test]
fn native_non_solid_brush_is_rejected_before_drawing_and_cannot_become_opaque_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let real = target()?;
    // SAFETY: These owned resources are created on an unbound target without native presentation.
    let gradient = unsafe {
        let stops = real.CreateGradientStopCollection(
            &[
                D2D1_GRADIENT_STOP { position: 0.0, color: encoded(1) },
                D2D1_GRADIENT_STOP { position: 1.0, color: encoded(2) },
            ],
            D2D1_GAMMA_2_2,
            D2D1_EXTEND_MODE_CLAMP,
        )?;
        real.CreateLinearGradientBrush(
            &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: windows_numerics::Vector2::default(),
                endPoint: windows_numerics::Vector2 { X: 1.0, Y: 1.0 },
            },
            None,
            &stops,
        )?
    };
    let proxy = Proxy::new(real, vec![PaintColor { token: 1, rgba: [255; 4] }])?;
    let brush: ID2D1Brush = gradient.cast()?;
    let observed = Cell::new(false);
    proxy.shared.paint("fixture", &(&brush).into(), || observed.set(true));
    assert!(!observed.get());
    assert_eq!(proxy.shared.failure.get(), windows::Win32::Foundation::E_NOINTERFACE);
    assert!(proxy.finish().is_err());
    Ok(())
}
