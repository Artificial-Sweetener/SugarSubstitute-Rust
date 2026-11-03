// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Plan bounded sparse reads only where nearest reconstruction consumes original raster pixels.
use super::CoverageSource;
use super::*;
use crate::{AssetId, RasterSamples, Region};

impl Graph {
    /// Return exact source addressing for a large nearest reduction directly over a raster asset.
    /// Other operations use ordinary tiled dependencies, preserving intermediate image operations.
    /// # Errors
    /// Rejects foreign nodes and invalid output regions; preserves sampling failures.
    pub fn sampled_source(
        &self,
        node: NodeId,
        region: Region,
    ) -> Result<Option<(AssetId, RasterSamples)>, ImageError> {
        if region.intersection(self.extent(node)?.region()) != Some(region) {
            return Err(ImageError::Invalid("Sparse raster output lies outside its node"));
        }
        let Some((asset, extent, sampling)) = self.sampled_source_definition(node)? else {
            return Ok(None);
        };
        Ok(Some((asset, RasterSamples::new(extent, region, sampling)?)))
    }

    /// Return exact sample addressing for a large nearest reduction directly over coverage, so
    /// previews read only the coverage samples they show.
    /// # Errors
    /// Rejects foreign nodes and invalid output regions; preserves sampling failures.
    pub fn sampled_coverage(
        &self,
        node: NodeId,
        region: Region,
    ) -> Result<Option<(CoverageSource, RasterSamples)>, ImageError> {
        if region.intersection(self.extent(node)?.region()) != Some(region) {
            return Err(ImageError::Invalid("Sparse coverage output lies outside its node"));
        }
        let Operation::Resample { input, sampling, .. } = self.operation(node)? else {
            return Ok(None);
        };
        let Operation::Coverage { source, extent } = self.operation(input)? else {
            return Ok(None);
        };
        if sampling.filter() != Filter::Nearest || !sampling.large_reduction(extent) {
            return Ok(None);
        }
        Ok(Some((source, RasterSamples::new(extent, region, sampling)?)))
    }

    /// Keep sparse-leaf eligibility identical for individual reads and batch planning.
    pub(super) fn sampled_source_definition(
        &self,
        node: NodeId,
    ) -> Result<Option<(AssetId, Extent, Sampling)>, ImageError> {
        let Operation::Resample { input, sampling, .. } = self.operation(node)? else {
            return Ok(None);
        };
        let Operation::Raster { asset, extent } = self.operation(input)? else {
            return Ok(None);
        };
        if sampling.filter() != Filter::Nearest || !sampling.large_reduction(extent) {
            return Ok(None);
        }
        Ok(Some((asset, extent, sampling)))
    }
}
