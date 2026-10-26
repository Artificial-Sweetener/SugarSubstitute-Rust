// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Group DAG-addressed immutable sample reads by integrity-checked encoded source chunk.
use super::*;
use sugarsubstitute_image_ops::RasterSamples;

struct SparseBatch {
    right: usize,
    first_column: u32,
    last_column: u32,
    bytes: usize,
}

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
                let batch = self.sparse_batch(samples, left, stripe)?;
                crate::check(context)?;
                self.read_encoded(layout.offset(batch.first_column, stripe)?, batch.bytes)?;
                self.stats.read_bytes = self.stats.read_bytes.saturating_add(batch.bytes as u64);
                let mut encoded = 0;
                let mut x = left;
                for column in batch.first_column..=batch.last_column {
                    crate::check(context)?;
                    let chunk = layout.region(column, stripe)?;
                    let length = chunk.pixels() as usize * stride;
                    let expected: [u8; 32] = self.encoded.data[encoded..encoded + 32]
                        .try_into()
                        .map_err(|_| ImportError::Invalid("Source chunk checksum width changed"))?;
                    let payload = encoded + 32..encoded + 32 + length;
                    self.stats.chunks = self.stats.chunks.saturating_add(1);
                    let actual: [u8; 32] =
                        Sha256::digest(&self.encoded.data[payload.clone()]).into();
                    if actual != expected {
                        return Err(ImportError::Corrupt(Box::new(ImportError::Invalid(
                            "Reconstructed source chunk failed integrity verification",
                        ))));
                    }
                    let right = x + samples.columns()[x..batch.right]
                        .partition_point(|&sample| sample / TILE_EDGE == column);
                    for y in top..bottom {
                        crate::check(context)?;
                        for sample in x..right {
                            let source = ((samples.rows()[y] - chunk.y()) as usize
                                * chunk.width() as usize
                                + (samples.columns()[sample] - chunk.x()) as usize)
                                * stride
                                + payload.start;
                            output[y * samples.columns().len() + sample] =
                                self.decode_pixel(&self.encoded.data[source..source + stride])?;
                        }
                    }
                    x = right;
                    encoded = payload.end;
                }
                if x != batch.right || encoded != batch.bytes {
                    return Err(ImportError::Invalid("Sparse source batch layout differs"));
                }
                left = batch.right;
            }
            top = bottom;
        }
        Ok(())
    }

    /// Group only physically adjacent touched chunks within the original bounded workspace.
    fn sparse_batch(
        &self,
        samples: &RasterSamples,
        left: usize,
        stripe: u32,
    ) -> Result<SparseBatch, ImportError> {
        let layout = self.descriptor.layout();
        let stride = layout.packing().bytes() as usize;
        let first_column = samples.columns()[left] / TILE_EDGE;
        let mut last_column = first_column;
        let mut right = left;
        let mut bytes = 0_usize;
        while right < samples.columns().len() {
            let column = samples.columns()[right] / TILE_EDGE;
            if right > left && last_column.checked_add(1) != Some(column) {
                break;
            }
            let group =
                samples.columns()[right..].partition_point(|&sample| sample / TILE_EDGE == column);
            let chunk = layout.region(column, stripe)?;
            let length = (chunk.pixels() as usize)
                .checked_mul(stride)
                .and_then(|length| length.checked_add(32))
                .ok_or(ImportError::Invalid("Sparse source batch size overflow"))?;
            let Some(next) = bytes.checked_add(length) else {
                return Err(ImportError::Invalid("Sparse source batch size overflow"));
            };
            if next > self.encoded.data.len() {
                if right == left {
                    return Err(ImportError::Invalid(
                        "Sparse source chunk exceeds bounded workspace",
                    ));
                }
                break;
            }
            bytes = next;
            right += group;
            last_column = column;
        }
        Ok(SparseBatch { right, first_column, last_column, bytes })
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
