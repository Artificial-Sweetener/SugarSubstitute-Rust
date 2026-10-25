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
            let linear = index as f64 / (LEVELS - 1) as f64;
            let encoded = if linear <= 0.0031308 {
                12.92 * linear
            } else {
                1.055 * linear.powf(1.0 / 2.4) - 0.055
            };
            table.push((encoded * 255.0).round().clamp(0.0, 255.0) as u8);
        }
        Ok(Self { table, _memory: claim })
    }
    /// Convert one already-composited opaque linear pixel to `[red, green, blue, 255]`.
    /// # Errors
    /// Rejects nonopaque input; composition over the chosen matte belongs to the image graph.
    pub fn encode(&self, pixel: Pixel) -> Result<[u8; 4], ImageError> {
        let channels = pixel.channels();
        if channels[3] != 1.0 {
            return Err(ImageError::Invalid("Display encoding requires an opaque composite"));
        }
        let mut result = [255; 4];
        for (output, input) in result[..3].iter_mut().zip(channels) {
            let scaled = input.clamp(0.0, 1.0) * (LEVELS - 1) as f32;
            let integer = scaled as u32;
            // The bounded nonnegative fraction preserves half-up rounding without scalar roundf.
            let index = (integer + u32::from(scaled - integer as f32 >= 0.5)) as usize;
            *output = self.table[index];
        }
        Ok(result)
    }
    /// Canonical transfer entries indexed by a nearest 16-bit normalized linear sample.
    /// The borrow cannot outlive the original execution-backed table ownership.
    pub fn table(&self) -> &[u8] {
        &self.table
    }
}
