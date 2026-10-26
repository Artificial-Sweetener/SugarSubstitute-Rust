// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separable Lanczos-3 with global sample phase and bounded tap tables.
use crate::kernel::{
    AxisKernel as Kernel, MAX_SAMPLING_TAPS, SampleTap as Tap, nearest_coordinate,
};
use crate::{Extent, ImageError, Pixel, Sampling, SamplingAxis, buffer::Buffer};
use sugarsubstitute_contracts::execution::JobContext;

/// Reconstruct a preview directly from admitted dependency pixels, without convolution scratch.
pub(crate) fn nearest(
    input: &Buffer,
    source: Extent,
    sampling: Sampling,
    output: &mut Buffer,
    context: &JobContext,
) -> Result<(), ImageError> {
    let coordinate = |value: u32, axis: usize, length: u32| {
        nearest_coordinate(
            sampling.unshifted_index(axis, value),
            length,
            sampling.scale()[axis],
            sampling.offset()[axis],
        )
    };
    for y in output.region.y()..output.region.bottom() {
        crate::check(context)?;
        let sy = coordinate(y, 1, source.height());
        for x in output.region.x()..output.region.right() {
            let sx = coordinate(x, 0, source.width());
            let from = (sy - input.region.y()) as usize * input.region.width() as usize
                + (sx - input.region.x()) as usize;
            let to = (y - output.region.y()) as usize * output.region.width() as usize
                + (x - output.region.x()) as usize;
            output.pixels[to] = input.pixels[from];
        }
    }
    Ok(())
}

/// Resample bounded dependency pixels; scratch storage and tap tables are already admitted.
pub(crate) fn render(
    input: &Buffer,
    source: Extent,
    sampling: Sampling,
    output: &mut Buffer,
    scratch: &mut Buffer,
    context: &JobContext,
) -> Result<(), ImageError> {
    let region = output.region;
    let mut horizontal = [Kernel { taps: [Tap::default(); MAX_SAMPLING_TAPS], length: 0 };
        crate::TILE_EDGE as usize];
    for (x, kernel) in horizontal[..region.width() as usize].iter_mut().enumerate() {
        *kernel = sampling.axis_kernel(
            SamplingAxis::Horizontal,
            region.x() + x as u32,
            source.width(),
        )?;
    }
    for y in input.region.y()..input.region.bottom() {
        crate::check(context)?;
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
        crate::check(context)?;
        let kernel =
            sampling.axis_kernel(SamplingAxis::Vertical, region.y() + y, source.height())?;
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
