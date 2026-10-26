// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain unpublished patch storage while a worker fills one bounded tile per step.
use super::*;
use sugarsubstitute_image_ops::{MAX_RASTER_SAMPLE_BATCH, RegionTiles};

pub(crate) struct SiblingRegions {
    regions: [Region; MAX_RASTER_SAMPLE_BATCH],
    length: usize,
}
impl SiblingRegions {
    pub(crate) fn as_slice(&self) -> &[Region] {
        &self.regions[..self.length]
    }
    pub(crate) const fn len(&self) -> usize {
        self.length
    }
}

pub(crate) struct Preparation {
    update: PreparedUpdate,
    patch: usize,
    tiles: Option<std::iter::Peekable<RegionTiles>>,
    failed: bool,
}
impl Preparation {
    pub(crate) fn new(update: UpdatePlan, memory: &MemoryLease) -> Result<Self, RenderError> {
        Ok(Self {
            update: PreparedUpdate::allocate(update, memory)?,
            patch: 0,
            tiles: None,
            failed: false,
        })
    }
    /// Peek a bounded left-to-right group without consuming its first output tile.
    pub(crate) fn sibling_regions(&mut self, maximum: usize) -> Option<SiblingRegions> {
        if self.failed {
            return None;
        }
        let region = self.update.patches.get(self.patch).and_then(Option::as_ref)?.region;
        let tiles = self.tiles.get_or_insert_with(|| region.tiles().peekable());
        let mut remaining = tiles.clone();
        let first = remaining.next()?;
        let mut result = SiblingRegions { regions: [first; MAX_RASTER_SAMPLE_BATCH], length: 1 };
        let maximum = maximum.clamp(1, MAX_RASTER_SAMPLE_BATCH);
        for region in remaining.take(maximum - 1) {
            if region.y() != first.y() || region.height() != first.height() {
                break;
            }
            result.regions[result.length] = region;
            result.length += 1;
        }
        Some(result)
    }
    pub(crate) fn fail(&mut self) {
        self.failed = true;
    }
    /// Return true only after every patch pixel has been produced and validated.
    pub(crate) fn step(
        &mut self,
        mut produce: impl FnMut(Region, Region, &mut [u32]) -> Result<(), RenderError>,
    ) -> Result<bool, RenderError> {
        if self.failed {
            return Err(RenderError::Invalid("Failed canvas preparation cannot resume"));
        }
        let Some(patch) = self.update.patches.get_mut(self.patch).and_then(Option::as_mut) else {
            return Ok(true);
        };
        let tiles = self.tiles.get_or_insert_with(|| patch.region.tiles().peekable());
        let tile = tiles.next().ok_or(RenderError::Invalid("Canvas preparation lost its tile"))?;
        self.failed = true;
        produce(tile, patch.region, &mut patch.pixels)?;
        for y in tile.y()..tile.bottom() {
            let start = (y - patch.region.y()) as usize * patch.region.width() as usize
                + (tile.x() - patch.region.x()) as usize;
            if patch.pixels[start..start + tile.width() as usize]
                .iter()
                .any(|pixel| pixel >> 24 != 255)
            {
                return Err(RenderError::Invalid(
                    "Canvas presentation requires opaque device pixels",
                ));
            }
        }
        self.failed = false;
        if tiles.peek().is_none() {
            self.patch += 1;
            self.tiles = None;
        }
        Ok(self.update.patches.get(self.patch).is_none_or(Option::is_none))
    }
    pub(crate) fn finish(self) -> PreparedUpdate {
        self.update
    }
}
