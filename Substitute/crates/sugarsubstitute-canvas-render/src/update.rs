// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate one revision dependency contract for prepared CPU and GPU updates.
use crate::RenderError;
use sugarsubstitute_canvas::{DamagePlan, FrameStamp};
use sugarsubstitute_contracts::execution::SurfaceId;
use sugarsubstitute_image_ops::Extent;

pub(crate) struct UpdatePlan {
    pub(crate) target: FrameStamp,
    pub(crate) base: Option<FrameStamp>,
    pub(crate) damage: DamagePlan,
}
impl UpdatePlan {
    pub(crate) fn new(
        target: FrameStamp,
        base: Option<FrameStamp>,
        damage: DamagePlan,
    ) -> Result<Self, RenderError> {
        if base.is_some_and(|base| {
            base.surface != target.surface || base.generation >= target.generation
        }) {
            return Err(RenderError::Invalid("Invalid canvas repair dependency"));
        }
        if base.is_none()
            && (damage.blit().is_some()
                || damage.dirty().regions().count() != 1
                || damage.dirty().regions().next() != Some(damage.dirty().surface().region()))
        {
            return Err(RenderError::Invalid("First canvas frame must prepare the full surface"));
        }
        Ok(Self { target, base, damage })
    }
    pub(crate) fn validate(
        &self,
        identity: SurfaceId,
        extent: Extent,
        current: Option<FrameStamp>,
    ) -> Result<(), RenderError> {
        if self.target.surface != identity || self.damage.dirty().surface() != extent {
            return Err(RenderError::Invalid("Canvas update targets a different surface"));
        }
        if current.is_some_and(|current| current.generation >= self.target.generation) {
            return Err(RenderError::StaleTarget);
        }
        if self.base.is_some() && self.base != current {
            return Err(RenderError::StaleBase);
        }
        Ok(())
    }
}
