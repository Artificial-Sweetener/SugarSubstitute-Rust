// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Evaluate only the requested Gaussian output and its declared finite dependency halo.
use super::*;
use crate::{GaussianBlur, MAX_GAUSSIAN_TAPS};

impl<S: RasterSource> Evaluator<S> {
    /// Two separable passes share the graph's coefficients, edge rule and admitted scratch.
    pub(super) fn blur(
        &mut self,
        input: NodeId,
        blur: GaussianBlur,
        output: &mut Buffer,
        context: &JobContext,
    ) -> Result<(), ImageError> {
        let extent = self.graph.extent(input)?;
        let region = output.region;
        let dependency = blur.expanded(region, extent)?;
        let input = self.input(input, dependency, context)?;
        let mut scratch = self.allocate(Region::new(
            region.x(),
            dependency.y(),
            region.width(),
            dependency.height(),
        )?)?;
        let _coefficients = self.claim((MAX_GAUSSIAN_TAPS * size_of::<f32>()) as u64)?;
        let weights = blur.weights();
        let radius = i64::from(blur.radius());
        let count = (2 * radius + 1) as usize;
        let width = region.width() as usize;
        let mut sums = [[0.0_f32; 4]; TILE_EDGE as usize];
        let sums = &mut sums[..width];
        let source_rows = input.pixels.chunks_exact(dependency.width() as usize);
        for (source_row, filtered) in source_rows.zip(scratch.pixels.chunks_exact_mut(width)) {
            check(context)?;
            sums.fill([0.0; 4]);
            for (index, weight) in weights[..count].iter().enumerate() {
                // Where the whole row's samples lie inside the image they are one shifted slice;
                // at an image edge each sample clamps on its own.
                let first = i64::from(region.x()) + index as i64 - radius;
                if first >= 0 && first + width as i64 <= i64::from(extent.width()) {
                    let start = (first - i64::from(dependency.x())) as usize;
                    crate::rows::accumulate(sums, &source_row[start..start + width], *weight);
                } else {
                    for (x, sum) in sums.iter_mut().enumerate() {
                        let sample =
                            (first + x as i64).clamp(0, i64::from(extent.width() - 1)) as u32;
                        let pixel = source_row[(sample - dependency.x()) as usize];
                        for (channel, value) in sum.iter_mut().zip(pixel.channels()) {
                            *channel += value * weight;
                        }
                    }
                }
            }
            for (pixel, sum) in filtered.iter_mut().zip(sums.iter()) {
                *pixel = Pixel(*sum);
            }
        }
        for (y, finished) in
            (region.y()..region.bottom()).zip(output.pixels.chunks_exact_mut(width))
        {
            check(context)?;
            sums.fill([0.0; 4]);
            for (index, weight) in weights[..count].iter().enumerate() {
                let sample = (i64::from(y) + index as i64 - radius)
                    .clamp(0, i64::from(extent.height() - 1)) as u32;
                let start = (sample - dependency.y()) as usize * width;
                crate::rows::accumulate(sums, &scratch.pixels[start..start + width], *weight);
            }
            for (pixel, sum) in finished.iter_mut().zip(sums.iter()) {
                *pixel = Pixel::filtered(*sum)?;
            }
        }
        Ok(())
    }
}
