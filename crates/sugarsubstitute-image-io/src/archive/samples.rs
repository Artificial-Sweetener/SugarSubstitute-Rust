// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Group DAG-addressed immutable sample reads by integrity-checked encoded source chunk.
use super::*;
use sugarsubstitute_image_ops::RasterSamples;

impl<R: Read + Seek> RasterArchive<R> {
    fn read_sparse(
        &mut self,
        samples: &RasterSamples,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImportError> {
        let layout = self.descriptor.layout();
        if samples.source() != layout.extent() || output.len() as u64 != samples.region().pixels() {
            return Err(ImportError::Invalid("Sparse source dimensions or destination differ"));
        }
        let stride = layout.packing().bytes() as usize;
        let mut top = 0;
        while top < samples.rows().len() {
            let stripe = samples.rows()[top] / layout.rows();
            let bottom =
                top + samples.rows()[top..].partition_point(|&row| row / layout.rows() == stripe);
            let mut left = 0;
            while left < samples.columns().len() {
                let column = samples.columns()[left] / TILE_EDGE;
                let right =
                    left + samples.columns()[left..].partition_point(|&x| x / TILE_EDGE == column);
                let chunk = self.read_chunk(column, stripe, context)?;
                for y in top..bottom {
                    crate::check(context)?;
                    for x in left..right {
                        let source = ((samples.rows()[y] - chunk.y()) as usize
                            * chunk.width() as usize
                            + (samples.columns()[x] - chunk.x()) as usize)
                            * stride;
                        output[y * samples.columns().len() + x] =
                            self.decode_pixel(&self.payload()[source..source + stride])?;
                    }
                }
                left = right;
            }
            top = bottom;
        }
        Ok(())
    }
}

impl<R: Read + Seek> RasterSource for RasterArchive<R> {
    fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &RasterSamples,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != self.descriptor.asset() {
            return Err(ImageError::Invalid("Raster source asset identity mismatch"));
        }
        self.read_sparse(samples, output, context)
            .map_err(|cause| ImageError::Source(Box::new(cause)))
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
