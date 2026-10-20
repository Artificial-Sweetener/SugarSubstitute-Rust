// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve RichEdit geometry while translating semantic paint tokens at publication.
//!
//! This UI-thread-affine COM boundary never exposes the real render target through QI.
//! Its owner begins/ends the real draw transaction and observes both EndDraw and finish.
use crate::DesktopError;
use std::{cell::Cell, rc::Rc};
use sugarsubstitute_contracts::transcript::MAX_STYLE_RUNS;
use windows::{
    Win32::{
        Foundation::{E_INVALIDARG, E_NOTIMPL},
        Graphics::Direct2D::{Common::*, *},
    },
    core::{HRESULT, Interface, Ref, implement},
};

#[cfg(test)]
#[path = "render_proxy/color_tests.rs"]
mod color_tests;
#[path = "render_proxy/diagnostics.rs"]
mod diagnostics;
#[path = "render_proxy/glyphs.rs"]
mod glyphs;
use diagnostics::{Context, Detail, GlyphFacts, NativeFailure};

/// Project bounded native paint facts without formatting text or walking arbitrary error sources.
pub(crate) fn diagnostic_code(
    cause: &(dyn std::error::Error + 'static),
) -> Option<crate::console_failure_diagnostic::ConsoleFailureCode> {
    use crate::console_failure_diagnostic::ConsoleFailureCode as Code;
    let failure = cause.downcast_ref::<NativeFailure>()?;
    Some(match failure.context.detail {
        Detail::Color { token: Some(token), .. } => Code::UnknownPaintToken(token),
        Detail::Color { token: None, .. } => Code::InvalidPaintColor,
        Detail::GlyphFormat { formats, .. } => Code::UnsupportedGlyphFormat(formats),
        Detail::Unsupported => Code::UnsupportedPaint,
        _ => Code::Hresult(failure.cause.code().0),
    })
}
#[path = "render_proxy/target.rs"]
mod target;
#[cfg(test)]
#[path = "render_proxy/tests.rs"]
mod tests;

/// An exact native COLORREF token and its straight, independently specified RGBA paint.
pub(super) struct PaintColor {
    pub(super) token: u32,
    pub(super) rgba: [u8; 4],
}

/// Each style can supply foreground/background semantics, plus defaults and selection.
pub(super) const MAX_PAINT_COLORS: usize = 2 * MAX_STYLE_RUNS + 4;

/// Additional admitted storage retained by the caller through final COM draw retirement.
///
/// Includes maximum vector capacity and this proxy's COM/Rc metadata; native document and
/// pixel-target allocations belong to their separately admitted authoritative owners.
pub(super) const PAINT_STORAGE_BYTES: u64 = (MAX_PAINT_COLORS * size_of::<PaintColor>()
    + size_of::<Shared>()
    + size_of::<Bridge_Impl>()
    + size_of::<Proxy>()
    + 2 * size_of::<usize>()) as u64;

/// Own the intercepted interface and its bounded immutable token table until draw completion.
pub(super) struct Proxy {
    interface: ID2D1RenderTarget,
    shared: Rc<Shared>,
}

impl Proxy {
    /// Adopt caller-charged storage; reject ambiguous or unbounded semantic dictionaries.
    pub(super) fn new(
        target: ID2D1RenderTarget,
        colors: Vec<PaintColor>,
    ) -> Result<Self, DesktopError> {
        if colors.capacity() > MAX_PAINT_COLORS {
            return Err(DesktopError::message(
                "console_paint_capacity",
                "semantic paint capacity exceeds its admitted bound",
            ));
        }
        validate(&colors).map_err(|cause| DesktopError::new("console_paint_tokens", cause))?;
        // SAFETY: The shared system factory is retained by this UI-thread-owned draw adapter.
        let factory = unsafe {
            windows::Win32::Graphics::DirectWrite::DWriteCreateFactory(
                windows::Win32::Graphics::DirectWrite::DWRITE_FACTORY_TYPE_SHARED,
            )
        }
        .map_err(|cause| DesktopError::new("console_color_glyph_factory", cause))?;
        let shared = Rc::new(Shared {
            target,
            colors,
            factory,
            failure: Cell::new(HRESULT(0)),
            context: Cell::new(Context::EMPTY),
            active_glyph: Cell::new(None),
        });
        let interface = Bridge { shared: shared.clone() }.into();
        Ok(Self { interface, shared })
    }

    /// Borrow only the proxy interface for this UI-thread draw transaction.
    pub(super) fn interface(&self) -> &ID2D1RenderTarget {
        &self.interface
    }

    /// Observe the first rejected paint or forwarded COM failure, including void callbacks.
    pub(super) fn finish(&self) -> Result<(), DesktopError> {
        self.shared.failure.get().ok().map_err(|cause| {
            DesktopError::new(
                "console_semantic_paint",
                NativeFailure { cause, context: self.shared.context.get() },
            )
        })
    }
}

/// Establish bounded binary-search invariants before exposing the COM callback.
fn validate(colors: &[PaintColor]) -> windows::core::Result<()> {
    if colors.is_empty()
        || colors.len() > MAX_PAINT_COLORS
        || colors.iter().any(|entry| entry.token > 0x00ff_ffff)
        || colors.windows(2).any(|pair| pair[0].token >= pair[1].token)
    {
        return Err(E_INVALIDARG.into());
    }
    Ok(())
}

/// Exact normalized byte colors are tokens; approximate color matching is forbidden.
fn token(color: D2D1_COLOR_F) -> windows::core::Result<u32> {
    if color.a != 1.0 {
        return Err(E_INVALIDARG.into());
    }
    let mut value = 0;
    for (shift, component) in [(0, color.r), (8, color.g), (16, color.b)] {
        if !component.is_finite() || !(0.0..=1.0).contains(&component) {
            return Err(E_INVALIDARG.into());
        }
        let byte = (component * 255.0).round() as u32;
        if component != byte as f32 / 255.0 && component != byte as f32 * (1.0 / 255.0) {
            return Err(E_INVALIDARG.into());
        }
        value |= byte << shift;
    }
    Ok(value)
}

/// UI-thread-owned draw state shared with the retained COM identity, without scheduler access.
struct Shared {
    target: ID2D1RenderTarget,
    colors: Vec<PaintColor>,
    factory: windows::Win32::Graphics::DirectWrite::IDWriteFactory2,
    failure: Cell<HRESULT>,
    context: Cell<Context>,
    active_glyph: Cell<Option<GlyphFacts>>,
}

impl Shared {
    /// Preserve the first native cause even when subsequent void callbacks also fail.
    #[cfg(test)]
    fn fail(&self, cause: HRESULT) -> windows::core::Error {
        self.fail_at("native_callback", cause, Detail::Native)
    }

    /// Store context only with the first failed HRESULT; later cleanup cannot replace either.
    fn fail_at(
        &self,
        method: &'static str,
        cause: HRESULT,
        detail: Detail,
    ) -> windows::core::Error {
        if self.failure.get().is_ok() {
            self.context.set(Context { method, detail, glyph: self.active_glyph.get() });
            self.failure.set(cause);
        }
        self.failure.get().into()
    }

    /// Reject unimplemented paint paths instead of forwarding unconverted color or pixels.
    fn unsupported<T>(&self, method: &'static str) -> windows::core::Result<T> {
        Err(self.fail_at(method, E_NOTIMPL, Detail::Unsupported))
    }

    /// Retain forwarded HRESULT failures independently of the caller's native draw result.
    fn record<T>(
        &self,
        method: &'static str,
        result: windows::core::Result<T>,
    ) -> windows::core::Result<T> {
        result.map_err(|cause| self.fail_at(method, cause.code(), Detail::Native))
    }

    /// Resolve semantic identity before converting caller-specified straight RGBA to floats.
    #[cfg(test)]
    fn color(&self, encoded: D2D1_COLOR_F) -> windows::core::Result<D2D1_COLOR_F> {
        self.color_at("decode_brush", encoded)
    }

    /// Preserve the actual native brush components when exact token validation rejects a paint.
    fn color_at(
        &self,
        method: &'static str,
        encoded: D2D1_COLOR_F,
    ) -> windows::core::Result<D2D1_COLOR_F> {
        let bits =
            [encoded.r.to_bits(), encoded.g.to_bits(), encoded.b.to_bits(), encoded.a.to_bits()];
        let key = token(encoded).map_err(|cause| {
            self.fail_at(method, cause.code(), Detail::Color { bits, token: None })
        })?;
        let index = self.colors.binary_search_by_key(&key, |entry| entry.token).map_err(|_| {
            self.fail_at(method, E_INVALIDARG, Detail::Color { bits, token: Some(key) })
        })?;
        let [r, g, b, a] = self.colors[index].rgba.map(|byte| f32::from(byte) / 255.0);
        Ok(D2D1_COLOR_F { r, g, b, a })
    }

    /// Temporarily substitute one solid brush while retaining native document geometry and opacity.
    fn paint(&self, method: &'static str, brush: &Ref<'_, ID2D1Brush>, draw: impl FnOnce()) {
        if self.failure.get().is_err() {
            return;
        }
        let prepared: windows::core::Result<_> = (|| {
            let raw = brush
                .ok()
                .map_err(|cause| self.fail_at(method, cause.code(), Detail::MissingBrush))?;
            let solid: ID2D1SolidColorBrush = raw
                .cast()
                .map_err(|cause| self.fail_at(method, cause.code(), Detail::NonSolidBrush))?;
            // SAFETY: The queried brush remains owned and valid throughout this synchronous call.
            let original = unsafe { solid.GetColor() };
            // SAFETY: Reading opacity has no native lifetime beyond this owned brush.
            let opacity = unsafe { solid.GetOpacity() };
            if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                return Err(self.fail_at(method, E_INVALIDARG, Detail::Opacity(opacity.to_bits())));
            }
            let mapped = self.color_at(method, original)?;
            Ok((solid, original, opacity, mapped))
        })();
        match prepared {
            Ok((solid, original, opacity, mapped)) => {
                // SAFETY: Direct2D synchronously consumes this borrowed color; the original
                // brush state is restored before returning to its native RichEdit owner.
                unsafe { solid.SetColor(&mapped) };
                draw();
                // SAFETY: The same owned brush remains live; no mutation escapes this call.
                unsafe {
                    solid.SetColor(&original);
                    solid.SetOpacity(opacity);
                }
            }
            Err(cause) => {
                self.fail_at(method, cause.code(), Detail::Native);
            }
        }
    }
}

#[implement(ID2D1RenderTarget, Agile = false)]
/// Expose only this proxy's render-target/resource/IUnknown identity to native callers.
struct Bridge {
    shared: Rc<Shared>,
}
