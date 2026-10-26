// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound retained whole-document and detailed camera coverage independently of execution.

use sugarsubstitute_canvas::{BackingCoverage, Viewport};
use sugarsubstitute_image_ops::Extent;

/// Symmetric retained-detail overscan in physical output pixels.
pub(super) const DETAIL_MARGIN: [u32; 2] = [128; 2];
/// Inner safety band that triggers repair before visible coverage is exhausted.
const DETAIL_REFRESH_MARGIN: [u32; 2] = [64; 2];
/// Maximum edge of the low-resolution whole-document fallback.
const FALLBACK_MAX_EDGE: u32 = 1024;

/// Independently retained backing selected for one coherent whole-output projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BackingTier {
    /// Bounded whole-document coverage used when detail cannot cover the target.
    Fallback,
    /// Full-quality overscanned coverage near the current camera.
    Detail,
}

/// Construct bounded whole-document coverage while preserving document aspect ratio.
pub(super) fn fallback(document: Extent) -> Result<BackingCoverage, Box<dyn std::error::Error>> {
    let longest = document.width().max(document.height());
    let edge = longest.min(FALLBACK_MAX_EDGE);
    let scaled = |value: u32| {
        u32::try_from((u64::from(value) * u64::from(edge)).div_ceil(u64::from(longest)).max(1))
    };
    let surface = Extent::new(scaled(document.width())?, scaled(document.height())?)?;
    let mut view = Viewport::new(surface)?;
    view.fit(document)?;
    Ok(BackingCoverage::new(view, [0; 2])?)
}

/// Construct overscanned detail coverage centered on the requested camera.
pub(super) fn detail(target: Viewport) -> Result<BackingCoverage, Box<dyn std::error::Error>> {
    Ok(BackingCoverage::new(target, DETAIL_MARGIN)?)
}

/// Refresh before the target consumes the outer half of retained detail overscan.
pub(super) fn detail_needs_refresh(
    backing: BackingCoverage,
    target: Viewport,
    document: Extent,
) -> Result<bool, Box<dyn std::error::Error>> {
    let guarded = BackingCoverage::new(target, DETAIL_REFRESH_MARGIN)?;
    if guarded.margin() != DETAIL_REFRESH_MARGIN {
        return Ok(false);
    }
    Ok(backing.crop(guarded.viewport(), document)?.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_fallback_covers_whole_document_after_pan_and_zoom_out()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = Extent::new(16_384, 8_192)?;
        let backing = fallback(document)?;
        assert_eq!(backing.viewport().surface(), Extent::new(1024, 512)?);

        let mut target = Viewport::new(Extent::new(640, 480)?)?;
        target.fit(document)?;
        target.zoom_at(0.5, [320.0, 240.0])?;
        target.pan_by([173.0, -91.0])?;
        assert!(backing.crop(target, document)?.is_some());
        Ok(())
    }

    #[test]
    fn detail_guard_covers_small_motion_but_hands_large_motion_to_fallback()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = Extent::new(4096, 4096)?;
        let mut target = Viewport::new(Extent::new(640, 480)?)?;
        target.reframe([1000.0, 1000.0], 1.0)?;
        let backing = detail(target)?;

        let mut inside = target;
        inside.pan_by([128.0, -128.0])?;
        assert!(backing.crop(inside, document)?.is_some());
        let mut outside = target;
        outside.pan_by([129.0, 0.0])?;
        assert!(backing.crop(outside, document)?.is_none());
        assert!(fallback(document)?.crop(outside, document)?.is_some());
        Ok(())
    }

    #[test]
    fn detail_refreshes_at_inner_guard_while_visible_coverage_remains_complete()
    -> Result<(), Box<dyn std::error::Error>> {
        let document = Extent::new(4096, 4096)?;
        let mut target = Viewport::new(Extent::new(640, 480)?)?;
        target.reframe([1000.0, 1000.0], 1.0)?;
        let backing = detail(target)?;

        target.pan_by([65.0, 0.0])?;
        assert!(backing.crop(target, document)?.is_some());
        assert!(detail_needs_refresh(backing, target, document)?);
        Ok(())
    }
}
