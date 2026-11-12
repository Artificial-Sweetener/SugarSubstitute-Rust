// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence viewport-local presentation effects independently from authored image/document state.
use super::*;
use sugarsubstitute_canvas_render::backdrop::{BackdropPlan, BackdropRegion};

/// At most two host-admitted physical regions and their session-owned monotonic revision.
#[derive(Default)]
pub(super) struct Backdrops {
    pub(super) regions: Vec<BackdropRegion>,
    pub(super) generation: u64,
    pub(super) presented: Option<u64>,
}
impl Backdrops {
    /// Capture the exact physical viewport and material for an immutable display transaction.
    pub(super) fn snapshot(
        &self,
        extent: sugarsubstitute_image_ops::Extent,
    ) -> Result<work::DisplayBackdrops, Box<dyn Error>> {
        Ok(work::DisplayBackdrops {
            plan: BackdropPlan::new(extent, &self.regions)?,
            generation: self.generation,
        })
    }
}
impl Session {
    /// Replace at most two physical viewport-local effects, retaining this session's revision.
    /// Identical geometry/material does no work. Source, view and overlay changes retain their
    /// own fences and reproject the same material over the new clean displayed pixels.
    /// # Errors
    /// Rejects invalid inventories, exhausted generations and updates after shutdown begins.
    #[cfg(feature = "native-composition")]
    pub(crate) fn set_backdrops(
        &mut self,
        regions: Vec<BackdropRegion>,
    ) -> Result<u64, Box<dyn Error>> {
        if self.closing {
            return Err("Canvas backdrop update arrived during shutdown".into());
        }
        BackdropPlan::new(self.document.tools.navigation().viewport().surface(), &regions)?;
        if self.backdrops.regions == regions {
            return Ok(self.backdrops.generation);
        }
        let generation = self
            .backdrops
            .generation
            .checked_add(1)
            .ok_or("Canvas backdrop generation exhausted")?;
        let now = self.navigation_time();
        self.abandon_cpu_frame(now)?;
        #[cfg(windows)]
        self.abandon_composition_frame(now)?;
        let mut bounded = Vec::with_capacity(2);
        bounded.extend(regions);
        self.backdrops.regions = bounded;
        self.backdrops.generation = generation;
        if let Some(displaying) = &self.displaying {
            displaying.job.cancel();
        }
        self.resize_preview_candidate(self.document.tools.navigation().viewport().surface())?;
        self.expose_pending = true;
        Ok(generation)
    }
    /// Revision actually accepted by the native presentation owner, never merely prepared.
    #[cfg(feature = "native-composition")]
    pub(crate) fn presented_backdrop_generation(&self) -> Option<u64> {
        self.backdrops.presented
    }
}
