// SPDX-License-Identifier: GPL-3.0-or-later
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
        for y in dependency.y()..dependency.bottom() {
            check(context)?;
            for x in region.x()..region.right() {
                let mut value = [0.0; 4];
                for (index, weight) in weights[..count].iter().enumerate() {
                    let sample = (i64::from(x) + index as i64 - radius)
                        .clamp(0, i64::from(extent.width() - 1))
                        as u32;
                    let pixel = input.pixels[(y - dependency.y()) as usize
                        * dependency.width() as usize
                        + (sample - dependency.x()) as usize];
                    for (channel, pixel) in value.iter_mut().zip(pixel.channels()) {
                        *channel += pixel * weight;
                    }
                }
                scratch.pixels[(y - dependency.y()) as usize * region.width() as usize
                    + (x - region.x()) as usize] = Pixel(value);
            }
        }
        for y in region.y()..region.bottom() {
            check(context)?;
            for x in region.x()..region.right() {
                let mut value = [0.0; 4];
                for (index, weight) in weights[..count].iter().enumerate() {
                    let sample = (i64::from(y) + index as i64 - radius)
                        .clamp(0, i64::from(extent.height() - 1))
                        as u32;
                    let pixel = scratch.pixels[(sample - dependency.y()) as usize
                        * region.width() as usize
                        + (x - region.x()) as usize];
                    for (channel, pixel) in value.iter_mut().zip(pixel.channels()) {
                        *channel += pixel * weight;
                    }
                }
                output.pixels[(y - region.y()) as usize * region.width() as usize
                    + (x - region.x()) as usize] = Pixel::filtered(value)?;
            }
        }
        Ok(())
    }
}
