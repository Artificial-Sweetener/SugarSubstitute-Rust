// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry captured pixels as the shared `CanvasPixelRaster` in canonical padded base64.

use super::{CanvasNativeError, wire};

/// The pixel encodings a capture can return.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasPixelFormat {
    /// Four bytes per pixel: red, green, blue, alpha; straight (unpremultiplied) alpha.
    ///
    /// A mask layer captured as a layer uses SugarCubes' coverage image encoding:
    /// red = green = blue = coverage and alpha = 255 − coverage.
    Rgba8,
    /// One coverage byte per pixel; 255 is fully covered.
    Coverage8,
}

impl CanvasPixelFormat {
    /// Return the bytes each pixel occupies.
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Rgba8 => 4,
            Self::Coverage8 => 1,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba8 => "rgba8",
            Self::Coverage8 => "coverage8",
        }
    }
}

/// Row-major, top-to-bottom pixels of one captured target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasPixelRaster {
    width: u32,
    height: u32,
    format: CanvasPixelFormat,
    pixels: Vec<u8>,
}

impl CanvasPixelRaster {
    /// Largest raster side, in pixels.
    pub const MAX_SIDE: u32 = 65_536;
    /// Largest raster payload, in bytes; its base64 text is the schema's 89,478,488 characters.
    pub const MAX_BYTES: usize = 64 * 1024 * 1024;

    /// Adopt tightly packed pixels for a `width` × `height` raster.
    ///
    /// # Errors
    /// Rejects a zero or oversized side, a byte length that does not match the extent and
    /// format exactly, or more than [`Self::MAX_BYTES`].
    pub fn new(
        width: u32,
        height: u32,
        format: CanvasPixelFormat,
        pixels: Vec<u8>,
    ) -> Result<Self, CanvasNativeError> {
        let side = 1..=Self::MAX_SIDE;
        let expected = usize::try_from(u64::from(width) * u64::from(height))
            .ok()
            .and_then(|count| count.checked_mul(format.bytes_per_pixel()));
        if !side.contains(&width)
            || !side.contains(&height)
            || expected != Some(pixels.len())
            || pixels.len() > Self::MAX_BYTES
        {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self { width, height, format, pixels })
    }

    /// Return the width in pixels.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Return the height in pixels.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Return the pixel encoding.
    pub const fn format(&self) -> CanvasPixelFormat {
        self.format
    }

    /// Borrow the tightly packed pixels.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Encode the shared shape; this allocates the base64 text once.
    pub(super) fn to_wire(&self) -> wire::CanvasPixelRaster {
        wire::CanvasPixelRaster {
            width: self.width.into(),
            height: self.height.into(),
            format: self.format.as_str().into(),
            data: encode_base64(&self.pixels),
        }
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode RFC 4648 standard base64 with `=` padding, the only spelling SugarCubes decodes.
pub(super) fn encode_base64(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = u32::from(chunk[0]) << 16
            | chunk.get(1).copied().map_or(0, u32::from) << 8
            | chunk.get(2).copied().map_or(0, u32::from);
        for index in 0..4 {
            if index <= chunk.len() {
                let sextet = (group >> (18 - 6 * index)) & 0x3f;
                text.push(char::from(ALPHABET[sextet as usize]));
            } else {
                text.push('=');
            }
        }
    }
    text
}
