// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Decode one lazy pose cache entry through the common bounded packaged-PNG owner.

use super::{CharacterError, POSE_SIZE, PoseAsset};
use crate::packaged_png::{PngError, PngRows};
use sugarsubstitute_ui::bitmap::Bitmap;

/// Preserve the asset identity while retaining at most one PNG decoder.
pub(super) struct PoseDecoder {
    asset: PoseAsset,
    rows: PngRows,
}
impl PoseDecoder {
    /// Validate the selected packaged pose and reserve its future cache entry.
    pub(super) fn new(asset: PoseAsset) -> Result<Self, CharacterError> {
        let rows =
            PngRows::new(asset.bytes, POSE_SIZE, POSE_SIZE).map_err(|error| map(asset, error))?;
        Ok(Self { asset, rows })
    }
    /// Decode at most one 386-pixel row or finish bounded PNG metadata parsing.
    pub(super) fn step(&mut self) -> Result<bool, CharacterError> {
        self.rows.step().map_err(|error| map(self.asset, error))
    }
    /// Move the reviewed premultiplied raster into its persistent cache slot.
    pub(super) fn finish(self) -> Result<Bitmap, CharacterError> {
        self.rows.finish().map_err(|error| map(self.asset, error))
    }
}
/// Keep existing public format/size errors and preserve all unexpected decoder causes.
fn map(asset: PoseAsset, error: PngError) -> CharacterError {
    match error {
        PngError::Size { width, height } => {
            CharacterError::AssetSize { name: asset.name, width, height }
        }
        PngError::Format => CharacterError::AssetFormat { name: asset.name },
        source => CharacterError::Decode { name: asset.name, source: Box::new(source) },
    }
}
