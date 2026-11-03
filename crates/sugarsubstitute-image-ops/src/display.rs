// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Canonical opaque sRGB display encoding with a shared bounded transfer lookup.
use crate::{ImageError, Pixel};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

const LEVELS: usize = 65536;
/// Linear-light to sRGB8 conversion using nearest 16-bit linear samples and rounded 8-bit output.
/// The same transfer bytes can be uploaded by a GPU adapter; no per-pixel transcendental is needed.
/// Display clips negative/HDR color to `[0, 1]`; authoritative image pixels retain their precision.
pub struct SrgbEncoder {
    table: Vec<u8>,
    _memory: MemoryClaim,
}
impl SrgbEncoder {
    /// Prepare the transfer table once on an admitted worker, retaining its allocation charge.
    /// # Errors
    /// Reports memory admission or allocator failure before exposing partial conversion state.
    pub fn new(memory: &MemoryLease) -> Result<Self, ImageError> {
        let claim = memory.claim(LEVELS as u64 + 128)?;
        let mut table = Vec::new();
        table.try_reserve_exact(LEVELS)?;
        for index in 0..LEVELS {
            table.push(transfer(index));
        }
        Ok(Self { table, _memory: claim })
    }
    /// Convert one already-composited opaque linear pixel to `[red, green, blue, 255]`.
    /// # Errors
    /// Rejects nonopaque input; composition over the chosen matte belongs to the image graph.
    pub fn encode(&self, pixel: Pixel) -> Result<[u8; 4], ImageError> {
        encode(pixel, |index| self.table[index])
    }
    /// Convert linear premultiplied channels to straight `[red, green, blue, alpha]` sRGB8, the
    /// interchange form of exported pixels. Coverage is clamped to `[0, 1]` and color to the
    /// display range; zero coverage encodes as transparent black.
    pub fn encode_straight(&self, channels: [f32; 4]) -> [u8; 4] {
        let alpha = if channels[3].is_finite() { channels[3].clamp(0.0, 1.0) } else { 0.0 };
        if alpha == 0.0 {
            return [0; 4];
        }
        let mut result = [0; 4];
        for (output, input) in result[..3].iter_mut().zip(channels) {
            *output = self.table[index(input / alpha)];
        }
        // The bounded product rounds half up without scalar roundf.
        result[3] = (alpha * 255.0 + 0.5) as u8;
        result
    }
    /// Encode an occasional opaque constant using the exact lookup quantization without allocation.
    /// Image loops use the retained encoder; backgrounds and display uniforms need no new table.
    /// # Errors
    /// Rejects nonopaque input using the same composition requirement as `encode`.
    pub fn encode_constant(pixel: Pixel) -> Result<[u8; 4], ImageError> {
        encode(pixel, transfer)
    }
    /// Canonical transfer entries indexed by a nearest 16-bit normalized linear sample.
    /// The borrow cannot outlive the original execution-backed table ownership.
    pub fn table(&self) -> &[u8] {
        &self.table
    }
}

/// Keep table construction and occasional constant conversion numerically identical.
fn transfer(index: usize) -> u8 {
    let linear = index as f64 / (LEVELS - 1) as f64;
    let encoded =
        if linear <= 0.0031308 { 12.92 * linear } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 };
    (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Share the 16-bit half-up index and opaque-composite contract across both conversion paths.
fn encode(pixel: Pixel, mut transfer: impl FnMut(usize) -> u8) -> Result<[u8; 4], ImageError> {
    let channels = pixel.channels();
    if channels[3] != 1.0 {
        return Err(ImageError::Invalid("Display encoding requires an opaque composite"));
    }
    let mut result = [255; 4];
    for (output, input) in result[..3].iter_mut().zip(channels) {
        *output = transfer(index(input));
    }
    Ok(result)
}

/// Nearest 16-bit transfer index of one linear channel clamped to the display range.
fn index(linear: f32) -> usize {
    let bounded = if linear.is_finite() { linear.clamp(0.0, 1.0) } else { 0.0 };
    let scaled = bounded * (LEVELS - 1) as f32;
    let integer = scaled as u32;
    // The bounded nonnegative fraction preserves half-up rounding without scalar roundf.
    (integer + u32::from(scaled - integer as f32 >= 0.5)) as usize
}
