// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Evaluate the shared finite Gaussian entirely on the selected image device.
use crate::{
    GpuError, GpuEvaluator,
    evaluate::image_error,
    kernel::{Computed, Parameters},
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{GaussianBlur, MAX_GAUSSIAN_TAPS, NodeId, Region};

impl<S> GpuEvaluator<S> {
    /// Gather the declared halo and retain both separable passes through device retirement.
    pub(crate) fn blur(
        &mut self,
        node: NodeId,
        input: NodeId,
        blur: GaussianBlur,
        region: Region,
        context: &JobContext,
    ) -> Result<Computed, GpuError> {
        let dependency = self.graph.required_inputs(node, region).map_err(image_error)?[0]
            .ok_or_else(|| GpuError::invalid("Missing GPU blur dependency"))?
            .1;
        let input = self.gather(input, dependency, context)?;
        let _tables = context
            .memory()
            .claim(((2 * MAX_GAUSSIAN_TAPS + 3) * 4) as u64)
            .map_err(|cause| GpuError::new("gpu_blur_coefficients", cause))?;
        let weights = blur.weights();
        let mut coefficients = [0; MAX_GAUSSIAN_TAPS + 3];
        coefficients[0] = blur.radius();
        coefficients[1] = 2 * blur.radius() + 1;
        for (destination, weight) in coefficients[2..].iter_mut().zip(weights) {
            *destination = weight.to_bits();
        }
        let scratch_region =
            Region::new(region.x(), dependency.y(), region.width(), dependency.height())
                .map_err(image_error)?;
        let scratch = self.dispatch(
            Parameters::new(10, scratch_region),
            scratch_region,
            [Some(&input), None, None],
            &coefficients,
            context,
        )?;
        drop(input);
        self.dispatch(
            Parameters::new(11, region),
            region,
            [Some(&scratch.tile), None, None],
            &coefficients,
            context,
        )
    }
}
