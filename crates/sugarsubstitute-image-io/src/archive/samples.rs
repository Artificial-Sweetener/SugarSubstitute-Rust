// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose integrity-checked single and batched sparse reads through the source boundary.
use super::*;
use sugarsubstitute_image_ops::{RasterSampleRequest, RasterSamples, validate_raster_sample_batch};

impl<R: Read + Seek> RasterSource for RasterArchive<R> {
    fn read_sample_batch(
        &mut self,
        asset: AssetId,
        requests: &mut [RasterSampleRequest<'_>],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != self.descriptor.asset() {
            return Err(ImageError::Invalid("Raster source asset identity mismatch"));
        }
        validate_raster_sample_batch(requests)?;
        if requests[0].samples.source() != self.descriptor.layout().extent() {
            return Err(ImageError::Invalid("Sparse source dimensions differ"));
        }
        self.read_sparse_batch(requests, context)
            .map_err(|cause| ImageError::Source(Box::new(cause)))
    }

    fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &RasterSamples,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        self.read_sample_batch(asset, &mut [RasterSampleRequest { samples, output }], context)
    }

    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != self.descriptor.asset() {
            return Err(ImageError::Invalid("Raster source asset identity mismatch"));
        }
        self.read_region(region, output, context)
            .map_err(|cause| ImageError::Source(Box::new(cause)))
    }
}
