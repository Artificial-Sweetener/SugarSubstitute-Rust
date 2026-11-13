// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Upload authoritative global sample coefficients and execute separable GPU reconstruction.
use crate::{
    GpuError, GpuEvaluator,
    evaluate::image_error,
    kernel::{Computed, Parameters},
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{Filter, NodeId, Region, Sampling, SamplingAxis};

impl<S> GpuEvaluator<S> {
    pub(crate) fn resample(
        &mut self,
        node: NodeId,
        input: NodeId,
        sampling: Sampling,
        region: Region,
        context: &JobContext,
    ) -> Result<Computed, GpuError> {
        let dependency = self.graph.required_inputs(node, region).map_err(image_error)?[0]
            .ok_or_else(|| GpuError::invalid("Missing GPU resampling dependency"))?
            .1;
        let source = self.graph.extent(input).map_err(image_error)?;
        let input = self.gather(input, dependency, context)?;
        let _tables = context
            .memory()
            .claim(32 * 1024)
            .map_err(|cause| GpuError::new("gpu_sampling_tables", cause))?;
        if sampling.filter() == Filter::Nearest {
            let mut table = [0; 256];
            for x in 0..region.width() {
                table[x as usize * 2] = sampling
                    .axis_kernel(SamplingAxis::Horizontal, region.x() + x, source.width())
                    .map_err(image_error)?
                    .taps()[0]
                    .coordinate()
                    - dependency.x();
            }
            for y in 0..region.height() {
                table[y as usize * 2 + 1] = sampling
                    .axis_kernel(SamplingAxis::Vertical, region.y() + y, source.height())
                    .map_err(image_error)?
                    .taps()[0]
                    .coordinate()
                    - dependency.y();
            }
            return self.dispatch(
                Parameters::new(8, region),
                region,
                [Some(&input), None, None],
                &table,
                context,
            );
        }
        let horizontal = table(
            sampling,
            SamplingAxis::Horizontal,
            region.x(),
            region.width(),
            source.width(),
            dependency.x(),
        )?;
        let scratch_region =
            Region::new(region.x(), dependency.y(), region.width(), dependency.height())
                .map_err(image_error)?;
        let scratch = self.dispatch(
            Parameters::new(6, scratch_region),
            scratch_region,
            [Some(&input), None, None],
            &horizontal,
            context,
        )?;
        drop(input);
        let vertical = table(
            sampling,
            SamplingAxis::Vertical,
            region.y(),
            region.height(),
            source.height(),
            dependency.y(),
        )?;
        self.dispatch(
            Parameters::new(7, region),
            region,
            [Some(&scratch.tile), None, None],
            &vertical,
            context,
        )
    }
}
fn table(
    sampling: Sampling,
    axis: SamplingAxis,
    start: u32,
    length: u32,
    source: u32,
    origin: u32,
) -> Result<[u32; 3584], GpuError> {
    let mut words = [0; 3584];
    for index in 0..length {
        let kernel = sampling.axis_kernel(axis, start + index, source).map_err(image_error)?;
        let base = index as usize * 28;
        words[base] = kernel.taps().len() as u32;
        for (index, tap) in kernel.taps().iter().enumerate() {
            words[base + 2 + index * 2] =
                tap.coordinate().checked_sub(origin).ok_or_else(|| {
                    GpuError::invalid("Sampling coefficient is outside its GPU dependency")
                })?;
            words[base + 3 + index * 2] = tap.weight().to_bits();
        }
    }
    Ok(words)
}
