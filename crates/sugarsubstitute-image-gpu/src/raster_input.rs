// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select host decoding or immutable device inputs without a synthetic CPU source.
use crate::{GpuError, GpuRasterBinding, evaluate::image_error};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{
    AssetId, ImageError, Pixel, RasterSampleRequest, RasterSamples, RasterSource, Region,
};

type Read<S> = fn(&mut S, AssetId, Region, &mut [Pixel], &JobContext) -> Result<(), ImageError>;
type Samples<S> =
    fn(&mut S, AssetId, &RasterSamples, &mut [Pixel], &JobContext) -> Result<(), ImageError>;
type Batch<S> =
    fn(&mut S, AssetId, &mut [RasterSampleRequest<'_>], &JobContext) -> Result<(), ImageError>;

pub(crate) struct HostReads<S> {
    read: Read<S>,
    samples: Samples<S>,
    batch: Batch<S>,
}
pub(crate) enum RasterInput<S> {
    Host { source: S, reads: HostReads<S> },
    Device { binding: GpuRasterBinding },
}
impl<S: RasterSource> RasterInput<S> {
    pub(crate) fn host(source: S) -> Self {
        Self::Host {
            source,
            reads: HostReads {
                read: S::read,
                samples: S::read_samples,
                batch: S::read_sample_batch,
            },
        }
    }
}
impl<S> RasterInput<S> {
    pub(crate) fn device(&self) -> Option<&GpuRasterBinding> {
        match self {
            Self::Device { binding } => Some(binding),
            Self::Host { .. } => None,
        }
    }
    pub(crate) fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), GpuError> {
        let Self::Host { source, reads } = self else {
            return Err(GpuError::invalid("Device raster cannot read host pixels"));
        };
        (reads.read)(source, asset, region, output, context).map_err(image_error)
    }
    pub(crate) fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &RasterSamples,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), GpuError> {
        let Self::Host { source, reads } = self else {
            return Err(GpuError::invalid("Device raster cannot read sparse host pixels"));
        };
        (reads.samples)(source, asset, samples, output, context).map_err(image_error)
    }
    pub(crate) fn read_sample_batch(
        &mut self,
        asset: AssetId,
        requests: &mut [RasterSampleRequest<'_>],
        context: &JobContext,
    ) -> Result<(), GpuError> {
        let Self::Host { source, reads } = self else {
            return Err(GpuError::invalid("Device raster cannot read host sample batches"));
        };
        (reads.batch)(source, asset, requests, context).map_err(image_error)
    }
}
