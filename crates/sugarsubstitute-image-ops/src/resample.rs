// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separable Lanczos-3 with global sample phase and bounded tap tables.
use crate::{Extent, ImageError, Pixel, buffer::Buffer};
use sugarsubstitute_contracts::execution::JobContext;

const MAX_TAPS: usize = 13;
#[derive(Clone, Copy, Default)]
struct Tap {
    coordinate: u32,
    weight: f32,
}
#[derive(Clone, Copy)]
struct Kernel {
    taps: [Tap; MAX_TAPS],
    length: usize,
}
impl Kernel {
    fn new(output: u32, source: u32, target: u32) -> Result<Self, ImageError> {
        let ratio = f64::from(source) / f64::from(target);
        let scale = ratio.max(1.0);
        let center = (f64::from(output) + 0.5) * ratio - 0.5;
        let first = (center - 3.0 * scale).ceil() as i64;
        let last = (center + 3.0 * scale).floor() as i64;
        let length = (last - first + 1) as usize;
        if length > MAX_TAPS {
            return Err(ImageError::Invalid("Resampling exceeds bounded Lanczos support"));
        }
        let mut taps = [Tap::default(); MAX_TAPS];
        let mut total = 0.0;
        for (i, tap) in taps[..length].iter_mut().enumerate() {
            let position = first + i as i64;
            let weight = lanczos((position as f64 - center) / scale);
            *tap = Tap {
                coordinate: position.clamp(0, i64::from(source) - 1) as u32,
                weight: weight as f32,
            };
            total += weight;
        }
        if !total.is_finite() || total.abs() < f64::EPSILON {
            return Err(ImageError::Invalid("Invalid Lanczos normalization"));
        }
        for tap in &mut taps[..length] {
            tap.weight = (f64::from(tap.weight) / total) as f32;
        }
        Ok(Self { taps, length })
    }
}

fn lanczos(distance: f64) -> f64 {
    if distance.abs() < 1e-12 {
        return 1.0;
    }
    if distance.abs() >= 3.0 {
        return 0.0;
    }
    let angle = std::f64::consts::PI * distance;
    (angle.sin() / angle) * ((angle / 3.0).sin() / (angle / 3.0))
}

/// Resample bounded dependency pixels; scratch storage and tap tables are already admitted.
pub(crate) fn render(
    input: &Buffer,
    source: Extent,
    target: Extent,
    output: &mut Buffer,
    scratch: &mut Buffer,
    context: &JobContext,
) -> Result<(), ImageError> {
    let region = output.region;
    let mut horizontal =
        [Kernel { taps: [Tap::default(); MAX_TAPS], length: 0 }; crate::TILE_EDGE as usize];
    for (x, kernel) in horizontal[..region.width() as usize].iter_mut().enumerate() {
        *kernel = Kernel::new(region.x() + x as u32, source.width(), target.width())?;
    }
    for y in input.region.y()..input.region.bottom() {
        crate::evaluate::check(context)?;
        let row = (y - input.region.y()) as usize * input.region.width() as usize;
        for (x, kernel) in horizontal[..region.width() as usize].iter().enumerate() {
            let mut value = [0.0; 4];
            for tap in &kernel.taps[..kernel.length] {
                let pixel = input.pixels[row + (tap.coordinate - input.region.x()) as usize];
                for (channel, input) in value.iter_mut().zip(pixel.0) {
                    *channel += input * tap.weight;
                }
            }
            scratch.pixels[(y - input.region.y()) as usize * region.width() as usize + x] =
                Pixel(value);
        }
    }
    for y in 0..region.height() {
        crate::evaluate::check(context)?;
        let kernel = Kernel::new(region.y() + y, source.height(), target.height())?;
        for x in 0..region.width() {
            let mut value = [0.0; 4];
            for tap in &kernel.taps[..kernel.length] {
                let pixel = scratch.pixels[(tap.coordinate - input.region.y()) as usize
                    * region.width() as usize
                    + x as usize];
                for (channel, input) in value.iter_mut().zip(pixel.0) {
                    *channel += input * tap.weight;
                }
            }
            output.pixels[y as usize * region.width() as usize + x as usize] =
                Pixel::filtered(value)?;
        }
    }
    Ok(())
}
