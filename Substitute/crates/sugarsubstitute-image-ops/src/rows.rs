// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Row-at-a-time arithmetic shared by the separable operations.
//!
//! An operation that weighs whole rows calls these instead of indexing one pixel at a time, so
//! the compiler sees straight slices it can vectorize and every operation gets the same speed.
use crate::Pixel;

/// Add `row` scaled by `weight` into `sums`, pixel for pixel. Taps applied in the same order
/// produce the same sums as a per-pixel loop over those taps.
pub(crate) fn accumulate(sums: &mut [[f32; 4]], row: &[Pixel], weight: f32) {
    for (sum, pixel) in sums.iter_mut().zip(row) {
        for (channel, value) in sum.iter_mut().zip(pixel.0) {
            *channel += value * weight;
        }
    }
}
