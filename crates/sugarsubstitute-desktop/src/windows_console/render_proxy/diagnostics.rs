// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded native paint evidence without allocating in void COM callbacks.
use std::{error::Error, fmt};

/// The first native operation and its rejected scalar evidence are immutable after failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Context {
    pub(super) method: &'static str,
    pub(super) detail: Detail,
    pub(super) glyph: Option<GlyphFacts>,
}
impl Context {
    pub(super) const EMPTY: Self =
        Self { method: "not_recorded", detail: Detail::Native, glyph: None };
}

/// Borrowed native run facts survive only as bounded scalars, never native font or array pointers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GlyphFacts {
    pub(super) count: u32,
    pub(super) em_bits: u32,
    pub(super) measuring: i32,
    pub(super) first: Option<u16>,
}

/// Native pointers are never retained; only bounded typed scalars cross callback retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Detail {
    Native,
    Unsupported,
    MissingBrush,
    NonSolidBrush,
    Color { bits: [u32; 4], token: Option<u32> },
    Opacity(u32),
    GlyphFormat { glyph: u16, formats: i32 },
    EmptyColorLayers,
    Palette([u32; 4]),
}
impl fmt::Display for Detail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native => f.write_str("native HRESULT failure"),
            Self::Unsupported => f.write_str("unsupported render operation"),
            Self::MissingBrush => f.write_str("null brush"),
            Self::NonSolidBrush => f.write_str("brush does not expose ID2D1SolidColorBrush"),
            Self::Color { bits, token } => {
                write!(f, "brush_rgba={:?} brush_bits={bits:08x?}", bits.map(f32::from_bits))?;
                match token {
                    Some(token) => write!(f, " unknown_token=0x{token:06x}"),
                    None => f.write_str(" invalid normalized opaque token"),
                }
            }
            Self::Opacity(bits) => {
                write!(f, "invalid brush_opacity={} bits=0x{bits:08x}", f32::from_bits(*bits))
            }
            Self::GlyphFormat { glyph, formats } => {
                write!(f, "unsupported glyph={glyph} image_formats=0x{formats:08x}")
            }
            Self::EmptyColorLayers => {
                f.write_str("successful color translation returned no layers")
            }
            Self::Palette(bits) => {
                write!(f, "invalid palette_rgba={:?} bits={bits:08x?}", bits.map(f32::from_bits))
            }
        }
    }
}

/// Add scalar callback context while preserving the original native HRESULT as the error source.
pub(super) struct NativeFailure {
    pub(super) cause: windows::core::Error,
    pub(super) context: Context,
}
impl fmt::Debug for NativeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NativeFailure({self})")
    }
}
impl fmt::Display for NativeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "method={} {}", self.context.method, self.context.detail)?;
        if let Some(glyph) = self.context.glyph {
            write!(
                f,
                " glyph_count={} font_em={} font_em_bits=0x{:08x} measuring={} first_glyph={:?}",
                glyph.count,
                f32::from_bits(glyph.em_bits),
                glyph.em_bits,
                glyph.measuring,
                glyph.first
            )?;
        }
        write!(f, "; {}", self.cause)
    }
}
impl Error for NativeFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
