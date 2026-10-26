// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bounded canonical raster reads supplied by storage adapters to image evaluation.

use crate::{AssetId, ImageError, Pixel, RasterSamples, Region, check};
use sugarsubstitute_contracts::execution::JobContext;

/// Maximum sparse source requests admitted into one storage-locality batch.
pub const MAX_RASTER_SAMPLE_BATCH: usize = 128;

/// One exact sparse sampling plan and its already-admitted row-major destination.
pub struct RasterSampleRequest<'a> {
    /// Original source coordinates selected by image-operation sampling mathematics.
    pub samples: &'a RasterSamples,
    /// Exact destination for `samples.region()` in row-major order.
    pub output: &'a mut [Pixel],
}

/// Validate a complete sparse source batch before an adapter may modify any destination.
///
/// Every request must use one source extent and provide exactly one destination pixel per
/// requested output pixel.
/// # Errors
/// Rejects empty or excessive inventories, mixed source extents and incorrect destinations.
pub fn validate_raster_sample_batch(
    requests: &[RasterSampleRequest<'_>],
) -> Result<(), ImageError> {
    let source = requests
        .first()
        .ok_or(ImageError::Invalid("Sparse raster batch is empty"))?
        .samples
        .source();
    if requests.len() > MAX_RASTER_SAMPLE_BATCH
        || requests.iter().any(|request| {
            request.samples.source() != source
                || request.output.len() as u64 != request.samples.region().pixels()
        })
    {
        return Err(ImageError::Invalid("Invalid sparse raster batch"));
    }
    Ok(())
}

/// Read-only asset boundary. Implementations provide exact canonical pixels in row-major order.
/// Reads execute on the caller's admitted worker, observe cancellation/deadlines, and bound their
/// own I/O/decoder workspace. They must never decode an unadmitted full image or spawn workers.
pub trait RasterSource {
    /// Fill exactly `region.pixels()` already-admitted output pixels for immutable asset content.
    /// # Errors
    /// Reports unavailable/corrupt sources and preserves original I/O/decoder causes.
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError>;

    /// Decode only the original pixels addressed by the DAG's bounded sparse sampling plan.
    /// Source adapters may group reads by encoded chunk without changing sample coordinates.
    /// The default reads individual canonical pixels, suitable for simple in-memory sources.
    /// # Errors
    /// Rejects an incorrect destination length and preserves source and control failures.
    fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &RasterSamples,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if output.len() as u64 != samples.region().pixels() {
            return Err(ImageError::Invalid("Invalid sparse raster destination"));
        }
        for (y, &row) in samples.rows().iter().enumerate() {
            for (x, &column) in samples.columns().iter().enumerate() {
                check(context)?;
                let index = y * samples.columns().len() + x;
                self.read(
                    asset,
                    Region::new(column, row, 1, 1)?,
                    std::slice::from_mut(&mut output[index]),
                    context,
                )?;
            }
        }
        Ok(())
    }

    /// Decode a bounded set of sparse plans while retaining each plan's exact output order.
    /// Storage adapters may reorder physical reads only after validating the complete batch.
    /// # Errors
    /// Validates every request before writing and preserves original sparse-read failures.
    fn read_sample_batch(
        &mut self,
        asset: AssetId,
        requests: &mut [RasterSampleRequest<'_>],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        validate_raster_sample_batch(requests)?;
        for request in requests {
            check(context)?;
            self.read_samples(asset, request.samples, request.output, context)?;
            check(context)?;
        }
        Ok(())
    }
}
